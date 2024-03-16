import easyflow
import time
import struct
from pathlib import Path


def video_loader_func():
    script_dir = Path(__file__).resolve().parent

    flow = easyflow.load_dataflow(script_dir / ".." / "dataflow.json5")
    sender = flow.build_sender("video_loader")

    while True:
        tick = int(time.time())
        payload = struct.pack("<L", tick)
        sender.send(payload)
        print(f"sent a video packet {tick}")
        time.sleep(1)
