#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["playwright==1.56.0"]
# ///
"""Capture real OBS virtual video and audio in Chromium and save evidence.

Start OBS's Virtual Camera with an audible source on its selected audio track.
Run with --chromium /path/to/chromium --output /path/to/evidence.
No fake media devices are used. Both OBS devices must be present.
"""

from __future__ import annotations

import argparse
import functools
import http.server
import json
import tempfile
import threading
from pathlib import Path

from playwright.sync_api import sync_playwright

PAGE = """<!doctype html><meta charset="utf-8"><title>OBS virtual camera verification</title>
<style>body{font:20px sans-serif;background:#101827;color:#eee;max-width:960px;margin:32px auto}
video{width:100%;max-height:540px;background:black}meter{width:100%;height:32px}
pre{white-space:pre-wrap;font-size:16px}h1{font-size:28px}</style>
<h1>OBS Virtual Camera + OBS Virtual Microphone</h1>
<video autoplay muted playsinline></video><p>Live microphone level</p><meter max="1"></meter>
<pre id="status">Waiting for real devices…</pre>
<script>
window.result = null;
async function capture() {
  const devices = await navigator.mediaDevices.enumerateDevices();
  const audio = devices.find(d => d.kind === 'audioinput' && /OBS Virtual Microphone/i.test(d.label));
  const video = devices.find(d => d.kind === 'videoinput' && /OBS.*(Virtual|Cam)|OBS Video/i.test(d.label));
  if (!audio || !video) throw Error('Missing OBS devices: ' + JSON.stringify(devices.map(d=>({kind:d.kind,label:d.label}))));
  const stream = await navigator.mediaDevices.getUserMedia({
    audio:{deviceId:{exact:audio.deviceId},echoCancellation:false,noiseSuppression:false,autoGainControl:false},
    video:{deviceId:{exact:video.deviceId}}
  });
  const element = document.querySelector('video');
  element.srcObject = stream;
  await element.play();
  const context = new AudioContext();
  await context.resume();
  const analyser = context.createAnalyser();
  context.createMediaStreamSource(stream).connect(analyser);
  const samples = new Float32Array(analyser.fftSize);
  let peakRms = 0;
  const firstTime = element.currentTime;
  function measure() {
    analyser.getFloatTimeDomainData(samples);
    const rms = Math.sqrt(samples.reduce((n,x)=>n+x*x,0)/samples.length);
    peakRms = Math.max(peakRms,rms);
    document.querySelector('meter').value = rms;
    window.result = {
      audio:stream.getAudioTracks()[0].label,video:stream.getVideoTracks()[0].label,
      audioState:stream.getAudioTracks()[0].readyState,videoState:stream.getVideoTracks()[0].readyState,
      width:element.videoWidth,height:element.videoHeight,
      videoSeconds:element.currentTime-firstTime,peakRms
    };
    document.querySelector('#status').textContent=JSON.stringify(window.result,null,2);
    requestAnimationFrame(measure);
  }
  measure();
}
capture().catch(e=>{window.captureError=String(e);document.querySelector('#status').textContent=String(e)});
</script>"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--chromium', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as directory:
        Path(directory, 'index.html').write_text(PAGE)
        handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
        with http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with sync_playwright() as playwright:
                    browser = playwright.chromium.launch(executable_path=args.chromium, headless=True,
                                                        args=['--autoplay-policy=no-user-gesture-required'])
                    try:
                        context = browser.new_context(permissions=['camera', 'microphone'],
                                                      viewport={'width': 1100, 'height': 950})
                        page = context.new_page()
                        page.goto(f'http://127.0.0.1:{server.server_port}')
                        page.wait_for_function('''window.captureError || (window.result &&
                          result.peakRms > 0.001 && result.videoSeconds > 1 &&
                          result.width > 0 && result.height > 0 &&
                          result.audioState === 'live' && result.videoState === 'live')''', timeout=30000)
                        error = page.evaluate('window.captureError')
                        if error:
                            raise RuntimeError(error)
                        result = page.evaluate('window.result')
                        page.screenshot(path=str(args.output / 'virtual-camera-audio.png'), full_page=True)
                        (args.output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
                        print(json.dumps(result, indent=2))
                    finally:
                        browser.close()
            finally:
                server.shutdown()
                thread.join()


if __name__ == '__main__':
    main()
