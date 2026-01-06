import argparse
import os
from pathlib import Path
from dotenv import load_dotenv, find_dotenv

# Load environment variables from .env
load_dotenv(find_dotenv())

# On systems with integrated + discrete AMD GPUs, use only the discrete one.
# This also suppresses the amdgpu-arch warning from ROCm SDK on Windows.
# The discrete GPU is typically at index 1 (integrated at 0).
if "CUDA_VISIBLE_DEVICES" not in os.environ:
    os.environ["CUDA_VISIBLE_DEVICES"] = "1"

# Enable experimental Flash/Mem Efficient Attention for ROCm (fixes warnings/slowness)
os.environ.setdefault("TORCH_ROCM_AOTRITON_ENABLE_EXPERIMENTAL", "1")

import torch

# Limit VRAM usage to ~16GB (0.7 of 24GB) or user specified fraction to prevent system freezes
# This must be done BEFORE any CUDA/ROCm tensors are allocated
if torch.cuda.is_available():
    # Only applies to the visible device (index 0 after masking)
    try:
        # Reserve some VRAM for display/OS if running on a single GPU workstation
        torch.cuda.set_per_process_memory_fraction(0.7, 0)
    except Exception:
        pass  # Ignore if not supported

from datasets import load_dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
)
from peft import LoraConfig, get_peft_model, TaskType
from trl import SFTTrainer, SFTConfig

# Default configuration
BASE_MODEL = "google/gemma-2-2b-it"
OUTPUT_DIR = "models/adapter"


def detect_device():
    """Detect best available device: CUDA > ROCm > DirectML > CPU.

    For multi-GPU systems, prefers discrete GPUs over integrated (APU) graphics.

    Returns:
        Tuple of (device, device_name, use_cpu_flag)
        - device: torch device string (e.g., "cuda:1")
        - device_name: human-readable name for logging
        - use_cpu_flag: whether to set use_cpu=True in SFTConfig
    """
    # Check for CUDA/ROCm first
    # ROCm uses HIP which presents as CUDA via torch.cuda.* APIs
    if torch.cuda.is_available():
        device_count = torch.cuda.device_count()

        # Find best GPU (prefer discrete over integrated)
        best_idx = 0
        for i in range(device_count):
            name = torch.cuda.get_device_name(i)
            # Integrated GPUs usually have "Radeon(TM) Graphics" or "Intel" in name
            # Discrete GPUs have model numbers like "RX 7900", "RTX 4090", etc.
            if "Radeon(TM) Graphics" not in name and "Intel" not in name.lower():
                best_idx = i
                break

        device_name = torch.cuda.get_device_name(best_idx)
        device_str = f"cuda:{best_idx}"

        # Set default device so model loading uses correct GPU
        torch.cuda.set_device(best_idx)

        # Distinguish ROCm from CUDA via torch.version.hip
        hip_version = getattr(torch.version, "hip", None)
        if hip_version:
            return device_str, f"ROCm ({device_name})", False
        else:
            return device_str, f"CUDA ({device_name})", False

    # Try DirectML (AMD/Intel on Windows, older fallback)
    try:
        import torch_directml

        dml_device = torch_directml.device()
        # Verify it actually works
        test_tensor = torch.zeros(1, device=dml_device)
        del test_tensor
        return dml_device, "DirectML (AMD/Intel GPU)", False
    except ImportError:
        pass
    except Exception as e:
        print(f"DirectML available but failed: {e}")

    # Fallback to CPU
    return "cpu", "CPU", True


def load_training_dataset(
    data_path: Path,
    eager: bool = False,
    chunk_size: int = 0,
    prefetch_count: int = 0,
):
    """Load training data from JSON or Cap'n Proto binary format.

    Supports:
      - .cpb.zip files (Cap'n Proto binary archive - recommended)
      - .cpb files (single Cap'n Proto binary file)
      - .zip files (Cap'n Proto binary archive)
      - .jsonl files (legacy JSON format)

    Args:
        data_path: Path to training data file
        eager: If True, load all data into memory (slower but allows shuffling).
        chunk_size: If > 0, use chunked prefetch (fast startup + fast training).
                   If 0 (default), use streaming (slow but memory-efficient).
        prefetch_count: If > 0, use true background prefetch with queue of this size.
                       Overlaps CPU loading with GPU training. Recommended: 1000.

    Returns a HuggingFace Dataset or IterableDataset with 'text' column ready for SFT.
    """
    path_str = str(data_path).lower()

    # Check for Cap'n Proto formats
    if (
        path_str.endswith(".cpb.zip")
        or path_str.endswith(".cpb")
        or (data_path.suffix.lower() == ".zip" and not path_str.endswith(".jsonl.zip"))
    ):
        if eager:
            # Eager mode: load all into memory (slower, allows full shuffle)
            from load_training_data import load_training_file, to_huggingface_dataset

            print(f"Loading Cap'n Proto binary (eager): {data_path}")
            samples = load_training_file(data_path)
            dataset = to_huggingface_dataset(samples)

            # Combine prompt + completion into 'text' for SFT
            def combine_text(example):
                return {"text": example["prompt"] + "\n" + example["completion"]}

            dataset = dataset.map(combine_text)
            print(f"Loaded {len(dataset)} samples")
        elif prefetch_count > 0:
            # True background prefetch: producer thread fills queue while GPU trains
            from load_training_data import to_huggingface_dataset_prefetch

            print(
                f"Loading Cap'n Proto binary (prefetch, queue={prefetch_count}): {data_path}"
            )
            dataset = to_huggingface_dataset_prefetch(
                data_path, prefetch_count=prefetch_count
            )
            print("Background prefetch enabled - CPU loads while GPU trains")
        elif chunk_size > 0:
            # Chunked prefetch: best of both worlds
            from load_training_data import to_huggingface_dataset_chunked

            print(
                f"Loading Cap'n Proto binary (chunked, size={chunk_size}): {data_path}"
            )
            dataset = to_huggingface_dataset_chunked(data_path, chunk_size=chunk_size)
            print("Chunked prefetch enabled - fast startup + fast training")
        else:
            # Streaming mode (default): memory-efficient, starts immediately
            from load_training_data import to_huggingface_dataset_streaming

            print(f"Loading Cap'n Proto binary (streaming): {data_path}")
            dataset = to_huggingface_dataset_streaming(data_path)
            print("Streaming enabled - training will start immediately")

    elif data_path.suffix.lower() in (".jsonl", ".json"):
        # Legacy JSON format
        print(f"Loading JSON: {data_path}")
        dataset = load_dataset("json", data_files=str(data_path), split="train")

        # Check if we need to combine prompt/completion
        if "text" not in dataset.column_names:
            if (
                "prompt" in dataset.column_names
                and "completion" in dataset.column_names
            ):

                def combine_text(example):
                    return {"text": example["prompt"] + "\n" + example["completion"]}

                dataset = dataset.map(combine_text)
            else:
                raise ValueError(
                    f"JSON file must have 'text' column or 'prompt'+'completion' columns"
                )
        print(f"Loaded {len(dataset)} samples")
    else:
        raise ValueError(
            f"Unsupported file format: {data_path.suffix}. Use .cpb.zip, .cpb, or .jsonl"
        )

    return dataset


def main():
    parser = argparse.ArgumentParser(description="Train EU4 AI using TRL/PEFT")
    parser.add_argument(
        "--data",
        required=True,
        type=Path,
        help="Path to training data (.cpb.zip, .cpb, or .jsonl)",
    )
    parser.add_argument(
        "--base-model", default=BASE_MODEL, help="Base HuggingFace model"
    )
    parser.add_argument(
        "--output", default=OUTPUT_DIR, help="Output directory for adapter"
    )
    parser.add_argument(
        "--epochs",
        type=int,
        default=1,
        help="Number of training epochs (eager mode only)",
    )
    parser.add_argument(
        "--max-steps",
        type=int,
        default=None,
        help="Max training steps (required for streaming, overrides --epochs)",
    )
    parser.add_argument(
        "--save-steps",
        type=int,
        default=None,
        help="Save checkpoint every N steps (default: max_steps/2)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=1,
        help="Training batch size per device (default: 1)",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=0,
        help="Dataloader workers for parallel data loading (default: 0)",
    )
    parser.add_argument(
        "--grad-accum",
        type=int,
        default=2,
        help="Gradient accumulation steps (default: 2)",
    )
    parser.add_argument(
        "--chunk-size",
        type=int,
        default=0,
        help="Chunk size for prefetch loading (0=streaming, >0=chunked). Recommended: 50000",
    )
    parser.add_argument(
        "--prefetch",
        type=int,
        default=0,
        help="True background prefetch queue size. Overlaps CPU loading with GPU training. Recommended: 1000",
    )
    parser.add_argument(
        "--eager",
        action="store_true",
        help="Force eager loading (slower, allows full shuffle)",
    )
    parser.add_argument(
        "--resume-from",
        type=Path,
        default=None,
        help="Resume training from a checkpoint directory",
    )

    parser.add_argument(
        "--learning-rate",
        type=float,
        default=2e-4,
        help="Initial learning rate (default: 2e-4)",
    )
    parser.add_argument(
        "--lora-r",
        type=int,
        default=16,
        help="LoRA attention dimension (rank). Higher = more parameters. (default: 16)",
    )
    parser.add_argument(
        "--lora-alpha",
        type=int,
        default=32,
        help="LoRA alpha scaling factor. usually 2x rank. (default: 32)",
    )
    parser.add_argument(
        "--lora-dropout",
        type=float,
        default=0.05,
        help="LoRA dropout probability (default: 0.05)",
    )
    parser.add_argument(
        "--weight-decay",
        type=float,
        default=0.01,
        help="Weight decay for optimizer (default: 0.01)",
    )
    parser.add_argument(
        "--lr-scheduler",
        type=str,
        default="linear",
        choices=["linear", "cosine", "cosine_with_restarts", "constant"],
        help="Learning rate scheduler type (default: linear)",
    )

    args = parser.parse_args()

    # Detect best available device
    device, device_name, use_cpu = detect_device()
    print(f"Using device: {device_name}")

    print(f"Loading base model: {args.base_model}")

    # Load Tokenizer
    tokenizer = AutoTokenizer.from_pretrained(args.base_model)
    tokenizer.padding_side = "right"  # Important for SFT
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    # Load Model
    # DirectML requires manual device placement, CUDA can use device_map="auto"
    is_directml = device not in ("cuda", "cpu")

    if is_directml:
        # DirectML: load to CPU first, then move to DML device
        model = AutoModelForCausalLM.from_pretrained(
            args.base_model,
            torch_dtype=torch.float32,
        )
        model = model.to(device)
    else:
        # CUDA or CPU: use device_map
        device_map = "auto" if device == "cuda" else "cpu"
        model = AutoModelForCausalLM.from_pretrained(
            args.base_model,
            device_map=device_map,
            torch_dtype=torch.float32,
        )

    # Configure LoRA
    peft_config = LoraConfig(
        task_type=TaskType.CAUSAL_LM,
        r=args.lora_r,
        lora_alpha=args.lora_alpha,
        target_modules=[
            "q_proj",
            "v_proj",
            "gate_proj",
            "up_proj",
            "down_proj",
        ],  # Gemma/Llama targets
        lora_dropout=args.lora_dropout,
    )

    model = get_peft_model(model, peft_config)
    model.print_trainable_parameters()

    # Load Dataset (streaming by default, --prefetch or --chunk-size for faster)
    dataset = load_training_dataset(
        args.data,
        eager=args.eager,
        chunk_size=args.chunk_size,
        prefetch_count=args.prefetch,
    )
    # All non-eager modes use IterableDataset (no __len__)
    needs_max_steps = not args.eager

    # IterableDataset requires max_steps (no len() available)
    if needs_max_steps and args.max_steps is None:
        print(
            "Warning: Streaming/chunked mode without --max-steps. Defaulting to 1000 steps."
        )
        max_steps = 1000
    else:
        max_steps = args.max_steps

    # Configure SFT Args (inherits from TrainingArguments)
    # CUDA (NVIDIA) supports bf16; ROCm Windows has issues with bf16, use fp32 or fp16
    is_cuda_device = isinstance(device, str) and device.startswith("cuda")
    is_rocm = getattr(torch.version, "hip", None) is not None

    use_bf16 = is_cuda_device and not is_rocm  # NVIDIA defaults to bf16
    use_fp16 = is_rocm  # AMD defaults to fp16 for better performance/compat on Windows

    # Determine save_steps
    if args.save_steps:
        save_steps = args.save_steps
    elif needs_max_steps and max_steps:
        save_steps = max_steps // 2
    else:
        save_steps = 500

    training_args = SFTConfig(
        output_dir=args.output,
        max_steps=max_steps if max_steps is not None else -1,
        num_train_epochs=args.epochs,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=args.grad_accum,
        learning_rate=args.learning_rate,
        weight_decay=args.weight_decay,
        lr_scheduler_type=args.lr_scheduler,
        logging_steps=10,
        save_steps=save_steps,
        bf16=use_bf16,
        fp16=use_fp16,
        report_to="none",  # Disable wandb unless explicitly configured
        dataset_text_field="text",
        max_length=2048,
        dataset_num_proc=1,  # Streaming doesn't support multiprocessing well
    )

    trainer = SFTTrainer(
        model=model,
        train_dataset=dataset,
        args=training_args,
    )

    print("Starting training...")
    if args.resume_from:
        print(f"Resuming from checkpoint: {args.resume_from}")
        trainer.train(resume_from_checkpoint=str(args.resume_from))
    else:
        trainer.train()

    print(f"Saving adapter to {args.output}")
    trainer.save_model(args.output)


if __name__ == "__main__":
    main()
