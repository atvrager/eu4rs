#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "google-generativeai",
#     "pillow",
#     "python-dotenv",
# ]
# ///
"""Analyze GUI rendering issues using Gemini."""

import os
import sys
import argparse
import google.generativeai as genai
from PIL import Image
from dotenv import load_dotenv

def analyze_issues(image_paths, issue_description):
    load_dotenv()
    api_key = os.getenv("GEMINI_API_KEY")
    if not api_key:
        print("Error: GEMINI_API_KEY not found in .env or environment.")
        sys.exit(1)

    genai.configure(api_key=api_key)
    model = genai.GenerativeModel('gemini-3-flash-preview')

    prompt = f"""You are a graphics engineer debugging a game UI renderer.

ISSUE REPORTED: {issue_description}

Analyze the provided screenshot(s) and identify:
1. The specific visual problem(s) described
2. What the rendering code is likely doing wrong
3. Suggested fixes (be specific about coordinates, offsets, UV calculations, etc.)

Focus on:
- Text positioning and alignment
- "Ghost" or phantom sprites (sprites rendering when they shouldn't)
- Z-order / layering issues
- Sprite frame selection (multi-frame sprites showing wrong frame)

Be concise and technical. Give actionable fixes.
"""

    images = [Image.open(path) for path in image_paths]
    response = model.generate_content([prompt] + images)
    print(response.text)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Analyze GUI rendering issues with Gemini")
    parser.add_argument("images", nargs="+", help="Image path(s) to analyze")
    parser.add_argument("--issue", "-i", required=True, help="Description of the issue")

    args = parser.parse_args()
    analyze_issues(args.images, args.issue)