import easyflow
import time
import struct
from pathlib import Path


def merger_func():
    script_dir = Path(__file__).resolve().parent
    flow = easyflow.load_dataflow(script_dir / ".." / "dataflow.json5")
    sender = flow.build_sender_to("merger", "OUTPUT")

    video_packet = None
    lidar_packet = None

    def try_send_packet():
        nonlocal video_packet
        nonlocal lidar_packet

        if video_packet is not None and lidar_packet is not None:
            merge_packet = struct.pack("<LL", video_packet, lidar_packet)
            ## TODO: This line panics
            # sender.send(merge_packet)
            print("sent a merged packet")

            video_packet = None
            lidar_packet = None

    def video_callback(payload):
        nonlocal video_packet
        (video_packet,) = struct.unpack("<L", payload)
        try_send_packet()

    def lidar_callback(payload):
        nonlocal lidar_packet
        (lidar_packet,) = struct.unpack("<L", payload)
        try_send_packet()

    video_listener = flow.listen_from("merger", "VIDEO", video_callback)
    lidar_listener = flow.listen_from("merger", "LIDAR", lidar_callback)

    while True:
        time.sleep(1)
