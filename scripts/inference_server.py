r"""
EU4 AI Inference Server

A TCP server that loads the trained model on ROCm and handles inference requests
from the Rust simulation. Keeps the model warm on GPU for fast responses.

Protocol (JSON over TCP, newline-delimited):
- Request:  {"prompt": "...", "max_tokens": 50}
- Response: {"response": "...", "inference_ms": 123}
- Error:    {"error": "message"}

Usage:
    cd scripts
    .venv-rocm\Scripts\Activate.ps1
    python inference_server.py --adapter ../models/adapter --port 9876
"""

import argparse
import json
import os
import socket
import time
from pathlib import Path
from threading import Thread, Lock

# ROCm setup - must happen before torch import
if "CUDA_VISIBLE_DEVICES" not in os.environ:
    os.environ["CUDA_VISIBLE_DEVICES"] = "1"  # Use discrete GPU
os.environ.setdefault("TORCH_ROCM_AOTRITON_ENABLE_EXPERIMENTAL", "1")

# Suppress noisy transformers warnings
os.environ.setdefault("TRANSFORMERS_VERBOSITY", "error")

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import PeftModel

import warnings

warnings.filterwarnings("ignore", message=".*torch_dtype.*is deprecated")


# Default configuration - use smaller model for fast inference
DEFAULT_BASE_MODEL = "google/gemma-3-270m"
DEFAULT_PORT = 9876
DEFAULT_HOST = "127.0.0.1"


class InferenceServer:
    """TCP server for LLM inference with ROCm acceleration."""

    def __init__(
        self,
        base_model: str,
        adapter_path: Path | None,
        host: str = DEFAULT_HOST,
        port: int = DEFAULT_PORT,
    ):
        self.host = host
        self.port = port
        self.model = None
        self.tokenizer = None
        self.device = None
        self.lock = Lock()  # Serialize inference requests

        self._load_model(base_model, adapter_path)

    def _detect_device(self) -> tuple[str, str]:
        """Detect best available device, returns (device_str, device_name)."""
        if torch.cuda.is_available():
            device_count = torch.cuda.device_count()
            best_idx = 0

            for i in range(device_count):
                name = torch.cuda.get_device_name(i)
                # Prefer discrete over integrated
                if "Radeon(TM) Graphics" not in name and "Intel" not in name.lower():
                    best_idx = i
                    break

            device_name = torch.cuda.get_device_name(best_idx)
            device_str = f"cuda:{best_idx}"
            torch.cuda.set_device(best_idx)

            # Limit VRAM to prevent system freeze
            try:
                torch.cuda.set_per_process_memory_fraction(0.7, best_idx)
            except Exception:
                pass

            hip_version = getattr(torch.version, "hip", None)
            backend = "ROCm" if hip_version else "CUDA"
            return device_str, f"{backend} ({device_name})"

        return "cpu", "CPU"

    def _load_model(self, base_model: str, adapter_path: Path | None):
        """Load the base model and optional LoRA adapter."""
        self.device, device_name = self._detect_device()
        print(f"Using device: {device_name}")

        print(f"Loading base model: {base_model}")
        self.tokenizer = AutoTokenizer.from_pretrained(base_model)
        if self.tokenizer.pad_token is None:
            self.tokenizer.pad_token = self.tokenizer.eos_token

        # Load with appropriate dtype for ROCm
        is_rocm = getattr(torch.version, "hip", None) is not None
        dtype = torch.float16 if is_rocm else torch.bfloat16

        self.model = AutoModelForCausalLM.from_pretrained(
            base_model,
            torch_dtype=dtype,
            device_map="auto" if self.device.startswith("cuda") else None,
        )

        if adapter_path and adapter_path.exists():
            print(f"Loading LoRA adapter: {adapter_path}")
            self.model = PeftModel.from_pretrained(
                self.model,
                str(adapter_path),
                torch_dtype=dtype,
            )
            # Merge adapter for faster inference
            print("Merging adapter weights...")
            self.model = self.model.merge_and_unload()

        self.model.eval()
        print("Model ready for inference")

    def generate(self, prompt: str, max_tokens: int) -> tuple[str, int]:
        """
        Generate response for a prompt.
        Returns (response_text, inference_time_ms).
        """
        start_time = time.perf_counter()

        with self.lock:
            inputs = self.tokenizer(prompt, return_tensors="pt")
            input_ids = inputs["input_ids"].to(self.model.device)
            attention_mask = inputs["attention_mask"].to(self.model.device)

            with torch.no_grad():
                outputs = self.model.generate(
                    input_ids,
                    attention_mask=attention_mask,
                    max_new_tokens=max_tokens,
                    do_sample=False,  # Greedy for determinism
                    pad_token_id=self.tokenizer.pad_token_id,
                    eos_token_id=self.tokenizer.eos_token_id,
                )

            # Decode only the generated part
            generated_ids = outputs[0][input_ids.shape[1] :]
            response = self.tokenizer.decode(generated_ids, skip_special_tokens=True)

        inference_ms = int((time.perf_counter() - start_time) * 1000)
        return response, inference_ms

    def handle_client(self, conn: socket.socket, addr):
        """Handle a single client connection."""
        print(f"Client connected: {addr}")

        try:
            buffer = b""
            while True:
                data = conn.recv(4096)
                if not data:
                    break

                buffer += data

                # Process complete JSON messages (newline-delimited)
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    if not line.strip():
                        continue

                    try:
                        request = json.loads(line.decode("utf-8"))
                        response = self._process_request(request)
                    except json.JSONDecodeError as e:
                        response = {"error": f"Invalid JSON: {e}"}
                    except Exception as e:
                        response = {"error": str(e)}

                    # Send response
                    response_bytes = json.dumps(response).encode("utf-8") + b"\n"
                    conn.sendall(response_bytes)

        except ConnectionResetError:
            pass
        finally:
            conn.close()
            print(f"Client disconnected: {addr}")

    def _process_request(self, request: dict) -> dict:
        """Process a single inference request."""
        if "prompt" not in request:
            return {"error": "Missing 'prompt' field"}

        prompt = request["prompt"]
        max_tokens = request.get("max_tokens", 50)

        try:
            response, inference_ms = self.generate(prompt, max_tokens)
            return {"response": response, "inference_ms": inference_ms}
        except Exception as e:
            return {"error": str(e)}

    def serve(self):
        """Start the TCP server."""
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind((self.host, self.port))
            server.listen(5)

            print(f"Inference server listening on {self.host}:{self.port}")
            print("Press Ctrl+C to stop")

            try:
                while True:
                    conn, addr = server.accept()
                    # Handle each client in a thread (inference is serialized by lock)
                    thread = Thread(target=self.handle_client, args=(conn, addr))
                    thread.daemon = True
                    thread.start()
            except KeyboardInterrupt:
                print("\nShutting down...")


def main():
    parser = argparse.ArgumentParser(
        description="EU4 AI Inference Server (ROCm accelerated)"
    )
    parser.add_argument(
        "--base-model",
        default=DEFAULT_BASE_MODEL,
        help=f"HuggingFace model ID (default: {DEFAULT_BASE_MODEL})",
    )
    parser.add_argument(
        "--adapter",
        type=Path,
        default=None,
        help="Path to LoRA adapter directory",
    )
    parser.add_argument(
        "--host",
        default=DEFAULT_HOST,
        help=f"Host to bind (default: {DEFAULT_HOST})",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=DEFAULT_PORT,
        help=f"Port to bind (default: {DEFAULT_PORT})",
    )
    args = parser.parse_args()

    server = InferenceServer(
        base_model=args.base_model,
        adapter_path=args.adapter,
        host=args.host,
        port=args.port,
    )
    server.serve()


if __name__ == "__main__":
    main()
