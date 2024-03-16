#!/usr/bin/env python3
import dataflow
import time
from pathlib import Path

script_dir = Path(__file__).resolve().parent

flow = dataflow.load_dataflow(script_dir / 'dataflow.json5')

def callback(payload):
    print(payload)
    tick = struct.unpack("<L", payload)
    # print(f"received {tick}")

listener = flow.listen_from('message-matcher', 'exchange-video-capture', callback)

while True:
    time.sleep(1)
