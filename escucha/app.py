import argparse
import configparser
import logging
import os
import select
import signal
import subprocess
import tempfile
import time
from dataclasses import dataclass
from shutil import which
from typing import Callable, Optional

from evdev import InputDevice, ecodes, list_devices
from faster_whisper import WhisperModel


CONFIG_DIR = os.path.join(os.path.expanduser("~"), ".config", "escucha")
CONFIG_PATH = os.path.join(CONFIG_DIR, "config.ini")
LOG_DIR = os.path.join(os.path.expanduser("~"), ".local", "state", "escucha")

logger = logging.getLogger("escucha")

StatusCallback = Callable[[str], None]
TextCallback = Callable[[str], None]


@dataclass
class Settings:
    key_name: str
    keyboard_device: str
    model: str
    language: str
    paste_method: str
    paste_hotkey: str
    clipboard_paste: str
    ydotool_key_delay_ms: int
    log_file: str
    log_level: str
    clipboard_paste_delay_ms: int


def load_settings() -> Settings:
    config = configparser.ConfigParser()
    config.read(CONFIG_PATH)

    key_name = config.get("dictate", "key", fallback="KEY_FN")
    keyboard_device = config.get("dictate", "keyboard_device", fallback="auto")
    model = config.get("dictate", "model", fallback="base.en")
    language = config.get("dictate", "language", fallback="en")
    paste_method = config.get("dictate", "paste_method", fallback="auto")
    paste_hotkey = config.get("dictate", "paste_hotkey", fallback="auto")
    clipboard_paste = config.get("dictate", "clipboard_paste", fallback="auto")
    ydotool_key_delay_ms = config.getint("dictate", "ydotool_key_delay_ms", fallback=1)
    clipboard_paste_delay_ms = config.getint("dictate", "clipboard_paste_delay_ms", fallback=75)
    log_file = config.get("dictate", "log_file", fallback="")
    log_level = config.get("dictate", "log_level", fallback="info")

    return Settings(
        key_name=key_name,
        keyboard_device=keyboard_device,
        model=model,
        language=language,
        paste_method=paste_method,
        paste_hotkey=paste_hotkey,
        clipboard_paste=clipboard_paste,
        ydotool_key_delay_ms=ydotool_key_delay_ms,
        clipboard_paste_delay_ms=clipboard_paste_delay_ms,
        log_file=log_file,
        log_level=log_level,
    )


def ensure_default_config() -> None:
    if os.path.exists(CONFIG_PATH):
        return
    os.makedirs(CONFIG_DIR, exist_ok=True)
    config = configparser.ConfigParser()
    config["dictate"] = {
        "key": "KEY_FN",
        "keyboard_device": "auto",
        "model": "base.en",
        "language": "en",
        "paste_method": "auto",
        "paste_hotkey": "auto",
        "clipboard_paste": "auto",
        "ydotool_key_delay_ms": "1",
        "clipboard_paste_delay_ms": "75",
        "log_file": "",
        "log_level": "info",
    }
    with open(CONFIG_PATH, "w", encoding="utf-8") as f:
        config.write(f)


def resolve_key_code(key_name: str) -> int:
    if not key_name.startswith("KEY_"):
        key_name = f"KEY_{key_name}"
    code = getattr(ecodes, key_name, None)
    if code is None:
        raise ValueError(f"Unknown key name: {key_name}")
    return code


def setup_logging(settings: Settings) -> None:
    level = logging.INFO
    level_name = settings.log_level.strip().lower()
    if level_name == "debug":
        level = logging.DEBUG
    elif level_name == "warning":
        level = logging.WARNING
    elif level_name == "error":
        level = logging.ERROR

    logger.setLevel(level)
    formatter = logging.Formatter("%(asctime)s %(levelname)s %(message)s")

    stream = logging.StreamHandler()
    stream.setFormatter(formatter)
    logger.addHandler(stream)

    if settings.log_file:
        os.makedirs(LOG_DIR, exist_ok=True)
        file_path = settings.log_file
        if file_path.lower() == "default":
            file_path = os.path.join(LOG_DIR, "escucha.log")
        file_handler = logging.FileHandler(file_path, encoding="utf-8")
        file_handler.setFormatter(formatter)
        logger.addHandler(file_handler)


def pick_keyboard_device(key_code: int, device_path: str) -> InputDevice:
    if device_path != "auto":
        return InputDevice(device_path)

    devices = [InputDevice(path) for path in list_devices()]
    for dev in devices:
        name = dev.name.lower()
        if "mouse" in name or "touchpad" in name:
            continue
        caps = dev.capabilities().get(ecodes.EV_KEY, [])
        if key_code in caps:
            return dev
    if devices:
        return devices[0]
    raise RuntimeError("No input devices found")


def start_recording() -> tuple[subprocess.Popen, str]:
    temp = tempfile.NamedTemporaryFile(prefix="whisper-fn-", suffix=".wav", delete=False)
    temp.close()
    cmd = [
        "arecord",
        "-q",
        "-f",
        "S16_LE",
        "-r",
        "16000",
        "-c",
        "1",
        "-t",
        "wav",
        temp.name,
    ]
    proc = subprocess.Popen(cmd)
    logger.debug("Started recording: %s", temp.name)
    return proc, temp.name


def stop_recording(proc: subprocess.Popen) -> None:
    if proc.poll() is not None:
        return
    proc.send_signal(signal.SIGINT)
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.terminate()


def load_model(model_name: str) -> WhisperModel:
    logger.info("Loading model: %s", model_name)
    return WhisperModel(model_name, device="cpu", compute_type="int8")


def transcribe(model: WhisperModel, wav_path: str, language: str) -> str:
    logger.info("Transcribing %s", wav_path)
    segments, _info = model.transcribe(wav_path, language=language)
    text = "".join(seg.text for seg in segments).strip()
    return " ".join(text.split())


def pick_paste_method(preferred: str) -> str:
    if preferred in {"xdotool", "wtype", "ydotool"}:
        return preferred

    wayland = bool(os.environ.get("WAYLAND_DISPLAY"))
    x11 = bool(os.environ.get("DISPLAY"))
    if wayland and which("ydotool"):
        return "ydotool"
    if wayland and which("wtype"):
        return "wtype"
    if x11 and which("xdotool"):
        return "xdotool"
    if which("ydotool"):
        return "ydotool"
    if which("wtype"):
        return "wtype"
    if which("xdotool"):
        return "xdotool"
    raise RuntimeError("No paste tool found (install ydotool, wtype, or xdotool)")


def resolve_paste_hotkey(paste_hotkey: str) -> str:
    combo = paste_hotkey.lower().replace(" ", "")
    if combo not in {"auto", "", "ctrl+v", "ctrl+shift+v"}:
        return "ctrl+v"
    if combo in {"auto", ""}:
        return "ctrl+v"
    return combo


def paste_text(
    text: str,
    method: str,
    paste_hotkey: str,
    clipboard_paste: str,
    ydotool_key_delay_ms: int,
    clipboard_paste_delay_ms: int,
) -> None:
    if not text:
        return
    logger.info("Paste start: method=%s clipboard=%s hotkey=%s delay=%sms", method, clipboard_paste, paste_hotkey, ydotool_key_delay_ms)
    if method == "xdotool":
        subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "1", "--", text], check=False)
    elif method == "wtype":
        result = subprocess.run(["wtype", "-d", "1", "--", text], check=False)
        if result.returncode == 0:
            return

        fallback = subprocess.run(["wtype", "-"], input=text.encode("utf-8"), check=False)
        if fallback.returncode == 0:
            return

        if which("wl-copy"):
            copy = subprocess.run(["wl-copy"], input=text.encode("utf-8"), check=False)
            if copy.returncode == 0:
                paste = subprocess.run(["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"], check=False)
                if paste.returncode == 0:
                    return

        raise RuntimeError("wtype failed to type or paste text")
    elif method == "ydotool":
        env = os.environ.copy()
        if "YDOTOOL_SOCKET" not in env and os.path.exists("/run/ydotoold.sock"):
            env["YDOTOOL_SOCKET"] = "/run/ydotoold.sock"
        if "YDOTOOL_SOCKET" not in env:
            user_socket = os.path.join("/run/user", str(os.getuid()), ".ydotool_socket")
            if os.path.exists(user_socket):
                env["YDOTOOL_SOCKET"] = user_socket
        allow_clipboard = clipboard_paste.lower() != "off"
        if allow_clipboard and which("wl-copy"):
            copy = subprocess.run(["wl-copy"], input=text.encode("utf-8"), check=False)
            logger.debug("wl-copy return code: %s", copy.returncode)
            if copy.returncode == 0:
                if clipboard_paste_delay_ms > 0:
                    time.sleep(clipboard_paste_delay_ms / 1000.0)
                combo = resolve_paste_hotkey(paste_hotkey)
                if combo == "ctrl+shift+v":
                    keys = ["29:1", "42:1", "47:1", "47:0", "42:0", "29:0"]
                else:
                    keys = ["29:1", "47:1", "47:0", "29:0"]
                paste = subprocess.run(
                    ["ydotool", "key", "-d", str(ydotool_key_delay_ms), *keys],
                    check=False,
                    env=env,
                )
                logger.debug("ydotool paste return code: %s", paste.returncode)
                if paste.returncode == 0:
                    return

        result = subprocess.run(
            ["ydotool", "type", "-d", str(ydotool_key_delay_ms), "--file", "-"],
            input=text.encode("utf-8"),
            check=False,
            env=env,
        )
        logger.debug("ydotool type return code: %s", result.returncode)
        if result.returncode != 0:
            raise RuntimeError("ydotool failed to type text")
    else:
        raise ValueError(f"Unknown paste method: {method}")


def list_input_devices() -> None:
    for path in list_devices():
        dev = InputDevice(path)
        print(f"{path} - {dev.name}")


class DictationService:
    def __init__(
        self,
        settings: Settings,
        on_status: Optional[StatusCallback] = None,
        on_text: Optional[TextCallback] = None,
        on_error: Optional[StatusCallback] = None,
    ) -> None:
        self.settings = settings
        self.on_status = on_status
        self.on_text = on_text
        self.on_error = on_error

        self.key_code = resolve_key_code(settings.key_name)
        self.device = pick_keyboard_device(self.key_code, settings.keyboard_device)
        self.paste_method = pick_paste_method(settings.paste_method)
        self.model = load_model(settings.model)

    def _status(self, message: str) -> None:
        if self.on_status:
            self.on_status(message)

    def _text(self, text: str) -> None:
        if self.on_text:
            self.on_text(text)

    def _error(self, message: str) -> None:
        if self.on_error:
            self.on_error(message)

    def run(self, stop_event) -> None:
        recording = False
        proc = None
        wav_path = None
        self._status("ready")
        logger.info("Service ready on %s key=%s method=%s", self.device.path, self.settings.key_name, self.paste_method)

        while not stop_event.is_set():
            try:
                readable, _w, _e = select.select([self.device.fd], [], [], 0.2)
            except OSError as exc:
                self._error(f"Input device error: {exc}")
                break

            if not readable:
                continue

            try:
                events = self.device.read()
            except OSError as exc:
                self._error(f"Input read error: {exc}")
                break

            for event in events:
                if event.type != ecodes.EV_KEY:
                    continue
                if event.code != self.key_code:
                    continue

                if event.value == 1 and not recording:
                    proc, wav_path = start_recording()
                    recording = True
                    self._status("recording")
                    logger.debug("Recording started")
                elif event.value == 0 and recording:
                    stop_recording(proc)
                    time.sleep(0.1)
                    self._status("transcribing")
                    try:
                        text = transcribe(self.model, wav_path, self.settings.language)
                        self._text(text)
                        paste_text(
                            text,
                            self.paste_method,
                            self.settings.paste_hotkey,
                            self.settings.clipboard_paste,
                            self.settings.ydotool_key_delay_ms,
                            self.settings.clipboard_paste_delay_ms,
                        )
                        logger.info("Paste done (%d chars)", len(text))
                    except Exception as exc:
                        self._error(f"Transcribe/paste error: {exc}")
                        logger.exception("Transcribe/paste error")
                    finally:
                        if wav_path and os.path.exists(wav_path):
                            os.unlink(wav_path)
                    recording = False
                    self._status("ready")

        if recording and proc is not None:
            stop_recording(proc)
            if wav_path and os.path.exists(wav_path):
                os.unlink(wav_path)


def run_daemon() -> None:
    ensure_default_config()
    settings = load_settings()
    setup_logging(settings)
    service = DictationService(settings)

    class _Stop:
        def __init__(self) -> None:
            self._stop = False

        def is_set(self) -> bool:
            return self._stop

    stop = _Stop()
    service.run(stop)


def main() -> None:
    parser = argparse.ArgumentParser(description="Hold-to-dictate Whisper daemon")
    parser.add_argument("--list-devices", action="store_true", help="List input devices and exit")
    parser.add_argument("--gui", action="store_true", help="Launch a troubleshooting GUI")
    args = parser.parse_args()

    if args.list_devices:
        list_input_devices()
        return

    if args.gui:
        from .gui import run_gui

        run_gui()
        return

    run_daemon()


if __name__ == "__main__":
    main()
