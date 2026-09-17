#!/usr/bin/env python3
"""Orbitty mouth sidecar — loopback only. Not in Orbitty.app."""

from __future__ import annotations

import os
import threading

from fastapi import FastAPI
from pydantic import BaseModel, Field

import engine

PORT = int(os.environ.get("ORBITTY_MOUTH_PORT", "13121"))
HOST = "127.0.0.1"

app = FastAPI(title="orbitty-mouth", docs_url=None, redoc_url=None)


class ClassifyIn(BaseModel):
    line: str = Field(default="", max_length=800)


@app.get("/health")
def health():
    return {
        "ok": True,
        "ready": engine.ready(),
        "error": engine.load_error(),
        "weights": engine.weights_path(),
    }


@app.post("/classify")
def classify(body: ClassifyIn):
    return engine.classify(body.line)


def _boot():
    engine.load_model()


def main():
    threading.Thread(target=_boot, name="mouth-load", daemon=True).start()
    import uvicorn

    uvicorn.run(app, host=HOST, port=PORT, log_level="warning")


if __name__ == "__main__":
    main()
