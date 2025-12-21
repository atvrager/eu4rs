import sys
import argparse
from pathlib import Path
import torch
from datasets import load_dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    BitsAndBytesConfig,
)
from peft import LoraConfig, get_peft_model, TaskType
from trl import SFTTrainer, SFTConfig

# Default configuration
BASE_MODEL = "google/gemma-2-2b-it"
OUTPUT_DIR = "models/adapter"


def main():
    parser = argparse.ArgumentParser(description="Train EU4 AI using TRL/PEFT")
    parser.add_argument("--data", required=True, help="Path to training data (.jsonl)")
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

    # Load Dataset
    print(f"Loading dataset: {args.data}")
    dataset = load_dataset("json", data_files=args.data, split="train")

    # Formatting function
    column_names = dataset.column_names
    if "text" not in column_names:

        def formatting_func(example):
            return {"text": example["prompt"] + example["completion"]}

        dataset = dataset.map(formatting_func)

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
