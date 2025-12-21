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


def load_training_dataset(data_path: Path):
    """Load training data from JSON or Cap'n Proto binary format.

    Supports:
      - .cpb.zip files (Cap'n Proto binary archive - recommended)
      - .cpb files (single Cap'n Proto binary file)
      - .zip files (Cap'n Proto binary archive)
      - .jsonl files (legacy JSON format)

    Returns a HuggingFace Dataset with 'text' column ready for SFT.
    """
    path_str = str(data_path).lower()

    # Check for Cap'n Proto formats
    if (
        path_str.endswith(".cpb.zip")
        or path_str.endswith(".cpb")
        or (data_path.suffix.lower() == ".zip" and not path_str.endswith(".jsonl.zip"))
    ):
        # Cap'n Proto binary format (preferred)
        from load_training_data import load_training_file, to_huggingface_dataset

        print(f"Loading Cap'n Proto binary: {data_path}")
        samples = load_training_file(data_path)
        dataset = to_huggingface_dataset(samples)

        # Combine prompt + completion into 'text' for SFT
        def combine_text(example):
            return {"text": example["prompt"] + "\n" + example["completion"]}

        dataset = dataset.map(combine_text)

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
    else:
        raise ValueError(
            f"Unsupported file format: {data_path.suffix}. Use .cpb.zip, .cpb, or .jsonl"
        )

    print(f"Loaded {len(dataset)} samples")
    return dataset


def main():
    parser = argparse.ArgumentParser(description="Train EU4 AI using TRL/PEFT")
    parser.add_argument(
        "--data",
        required=True,
        type=Path,
        help="Path to training data (.bin for Cap'n Proto, .jsonl for JSON)",
    )
    parser.add_argument(
        "--base-model", default=BASE_MODEL, help="Base HuggingFace model"
    )
    parser.add_argument(
        "--output", default=OUTPUT_DIR, help="Output directory for adapter"
    )
    parser.add_argument(
        "--epochs", type=int, default=1, help="Number of training epochs"
    )

    args = parser.parse_args()

    print(f"Loading base model: {args.base_model}")

    # Load Tokenizer
    tokenizer = AutoTokenizer.from_pretrained(args.base_model)
    tokenizer.padding_side = "right"  # Important for SFT
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    # Load Model (Quantized if possible, but CPU support varies)
    # For simplicity on generic hardware, load standard float32 or bfloat16
    # If CUDA is available, use it.
    device_map = "auto" if torch.cuda.is_available() else "cpu"
    print(f"Using device: {device_map}")

    model = AutoModelForCausalLM.from_pretrained(
        args.base_model,
        device_map=device_map,
        torch_dtype=torch.float32,  # CPU friendly
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

    # Load Dataset (supports both .bin and .jsonl)
    dataset = load_training_dataset(args.data)

    # Configure SFT Args (inherits from TrainingArguments)
    sft_config = SFTConfig(
        output_dir=args.output,
        num_train_epochs=args.epochs,
        per_device_train_batch_size=1,
        gradient_accumulation_steps=4,
        learning_rate=2e-4,
        logging_steps=10,
        save_strategy="epoch",
        use_cpu=not torch.cuda.is_available(),
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
