import argparse
from pathlib import Path
import torch
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
    """Detect best available device: CUDA > DirectML > CPU.

    Returns:
        Tuple of (device, device_name, use_cpu_flag)
        - device: torch device or DirectML device object
        - device_name: human-readable name for logging
        - use_cpu_flag: whether to set use_cpu=True in SFTConfig
    """
    # Try CUDA first (NVIDIA)
    if torch.cuda.is_available():
        device_name = torch.cuda.get_device_name(0)
        return "cuda", f"CUDA ({device_name})", False

    # Try DirectML (AMD/Intel on Windows)
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
        r=16,
        lora_alpha=32,
        target_modules=[
            "q_proj",
            "v_proj",
            "gate_proj",
            "up_proj",
            "down_proj",
        ],  # Gemma/Llama targets
        lora_dropout=0.05,
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
    # CUDA supports bf16 mixed precision; DirectML and CPU do not
    use_bf16 = device == "cuda"

    # Determine save_steps
    if args.save_steps:
        save_steps = args.save_steps
    elif needs_max_steps and max_steps:
        save_steps = max_steps // 2
    else:
        save_steps = 500

    sft_config = SFTConfig(
        output_dir=args.output,
        # Use max_steps for streaming/chunked, epochs for eager
        num_train_epochs=args.epochs if not needs_max_steps else 1,
        max_steps=max_steps if needs_max_steps else -1,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=4,
        learning_rate=2e-4,
        logging_steps=10,
        save_strategy="steps" if needs_max_steps else "epoch",
        save_steps=save_steps,
        use_cpu=use_cpu,  # True only for CPU, False for CUDA and DirectML
        # Mixed precision: bf16 for CUDA, fp32 for DirectML/CPU
        bf16=use_bf16,
        fp16=False,  # Prefer bf16 over fp16 when available
        dataloader_num_workers=args.workers,
        dataset_text_field="text",
    )

    trainer = SFTTrainer(
        model=model,
        train_dataset=dataset,
        args=sft_config,
    )

    print("Starting training...")
    trainer.train()

    print(f"Saving adapter to {args.output}")
    trainer.save_model(args.output)


if __name__ == "__main__":
    main()
