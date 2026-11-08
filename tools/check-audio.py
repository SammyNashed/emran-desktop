#!/usr/bin/env python3
"""Compares the Rust decoders with ffmpeg, sample by sample. Usage: check-audio.py <dump dir> <audio dir>"""
import array, math, subprocess, sys
FILES = {"ahmad_short": "ahmad_alnafees", "ahmad_full": "ahmad_alnafees_full", "mishary_short": "mishary_alafasy", "mishary_full": "mishary_alafasy_full",
         "mishary_fajr": "mishary_alafasy_fajr_full", "mishary_tathweeb": "mishary_alafasy_tathweeb", "saad_short": "saad_alghamdi", "saad_full": "saad_alghamdi_full"}
def floats(data):
    a = array.array("f"); a.frombytes(data[: len(data) - len(data) % 4]); return a
worst = 0.0
for name, source in FILES.items():
    ours = floats(open(f"{sys.argv[1]}/{name}.f32", "rb").read())
    ref = floats(subprocess.run(["ffmpeg", "-v", "error", "-i", f"{sys.argv[2]}/{source}.ogg", "-f", "f32le", "-acodec", "pcm_f32le", "-"], capture_output=True, check=True).stdout)
    n = min(len(ours), len(ref))
    signal = sum(ref[i] * ref[i] for i in range(n)) or 1e-12
    noise = sum((ours[i] - ref[i]) ** 2 for i in range(n)) or 1e-12
    snr = 10 * math.log10(signal / noise)
    worst = min(worst, snr) if worst else snr
    print(f"{name:18} ours {len(ours):>9} ffmpeg {len(ref):>9} samples  match {snr:6.1f} dB")
print("worst match:", round(worst, 1), "dB")
