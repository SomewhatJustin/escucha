import queue
import select
import threading
import tkinter as tk
from tkinter import ttk

from evdev import InputDevice, ecodes, list_devices

from .app import CONFIG_PATH, DictationService, ensure_default_config, load_settings, setup_logging


def run_gui() -> None:
    ensure_default_config()
    settings = load_settings()
    setup_logging(settings)

    root = tk.Tk()
    root.title("Escucha")
    root.geometry("520x420")

    status_var = tk.StringVar(value="stopped")
    last_text_var = tk.StringVar(value="")
    error_var = tk.StringVar(value="")
    key_test_status_var = tk.StringVar(value="stopped")
    key_test_device_var = tk.StringVar(value="auto")
    key_test_last_var = tk.StringVar(value="")

    messages: queue.Queue[tuple[str, str]] = queue.Queue()
    stop_event = threading.Event()
    key_test_stop = threading.Event()
    worker = None
    key_worker = None

    def on_status(msg: str) -> None:
        messages.put(("status", msg))

    def on_text(msg: str) -> None:
        messages.put(("text", msg))

    def on_error(msg: str) -> None:
        messages.put(("error", msg))

    def start_service() -> None:
        nonlocal worker
        if worker and worker.is_alive():
            return
        stop_event.clear()
        service = DictationService(settings, on_status=on_status, on_text=on_text, on_error=on_error)
        worker = threading.Thread(target=service.run, args=(stop_event,), daemon=True)
        worker.start()
        status_var.set("starting")

    def stop_service() -> None:
        stop_event.set()
        status_var.set("stopping")

    def list_key_devices() -> list[tuple[str, str]]:
        items: list[tuple[str, str]] = []
        for path in list_devices():
            dev = InputDevice(path)
            name = dev.name.lower()
            if "mouse" in name or "touchpad" in name:
                continue
            if ecodes.EV_KEY in dev.capabilities():
                items.append((path, dev.name))
        return items

    def pick_key_device_path() -> str:
        selection = device_choice.get()
        if selection and selection != "auto":
            return selection.split(" ", 1)[0]
        if settings.keyboard_device != "auto":
            return settings.keyboard_device
        devices = list_key_devices()
        if devices:
            return devices[0][0]
        raise RuntimeError("No keyboard device found for key test")

    def key_test_loop(device_path: str) -> None:
        try:
            dev = InputDevice(device_path)
            messages.put(("keytest_device", f"{device_path} - {dev.name}"))
        except Exception as exc:
            messages.put(("error", f"Key test device error: {exc}"))
            return

        while not key_test_stop.is_set():
            try:
                readable, _w, _e = select.select([dev.fd], [], [], 0.2)
            except OSError as exc:
                messages.put(("error", f"Key test select error: {exc}"))
                break
            if not readable:
                continue
            try:
                events = dev.read()
            except OSError as exc:
                messages.put(("error", f"Key test read error: {exc}"))
                break
            for event in events:
                if event.type != ecodes.EV_KEY:
                    continue
                name = ecodes.KEY.get(event.code, f"KEY_{event.code}")
                messages.put(("keytest", f"{name} (code {event.code}) value={event.value}"))

    def start_key_test() -> None:
        nonlocal key_worker
        if key_worker and key_worker.is_alive():
            return
        key_test_stop.clear()
        try:
            device_path = pick_key_device_path()
        except Exception as exc:
            error_var.set(str(exc))
            return
        key_worker = threading.Thread(target=key_test_loop, args=(device_path,), daemon=True)
        key_worker.start()
        key_test_status_var.set("listening")

    def stop_key_test() -> None:
        key_test_stop.set()
        key_test_status_var.set("stopping")

    def list_devices_text() -> str:
        lines = []
        try:
            for path in list_devices():
                dev = InputDevice(path)
                lines.append(f"{path} - {dev.name}")
        except Exception as exc:
            lines.append(f"Error: {exc}")
        return "\n".join(lines)

    def update_devices() -> None:
        devices_box.delete("1.0", tk.END)
        devices_box.insert(tk.END, list_devices_text())

    def process_messages() -> None:
        while True:
            try:
                kind, msg = messages.get_nowait()
            except queue.Empty:
                break
            if kind == "status":
                status_var.set(msg)
            elif kind == "text":
                last_text_var.set(msg)
            elif kind == "error":
                error_var.set(msg)
            elif kind == "keytest":
                key_test_last_var.set(msg)
            elif kind == "keytest_device":
                key_test_device_var.set(msg)
        root.after(200, process_messages)

    header = ttk.Label(root, text="Escucha", font=("TkDefaultFont", 14, "bold"))
    header.pack(pady=8)

    status_frame = ttk.Frame(root)
    status_frame.pack(fill=tk.X, padx=12, pady=4)
    ttk.Label(status_frame, text="Status:").pack(side=tk.LEFT)
    ttk.Label(status_frame, textvariable=status_var).pack(side=tk.LEFT, padx=6)

    error_frame = ttk.Frame(root)
    error_frame.pack(fill=tk.X, padx=12, pady=4)
    ttk.Label(error_frame, text="Error:").pack(side=tk.LEFT)
    ttk.Label(error_frame, textvariable=error_var, foreground="red").pack(side=tk.LEFT, padx=6)

    text_frame = ttk.Frame(root)
    text_frame.pack(fill=tk.X, padx=12, pady=4)
    ttk.Label(text_frame, text="Last text:").pack(side=tk.LEFT)
    ttk.Label(text_frame, textvariable=last_text_var, wraplength=420, justify="left").pack(side=tk.LEFT, padx=6)

    btn_frame = ttk.Frame(root)
    btn_frame.pack(fill=tk.X, padx=12, pady=8)
    ttk.Button(btn_frame, text="Start", command=start_service).pack(side=tk.LEFT)
    ttk.Button(btn_frame, text="Stop", command=stop_service).pack(side=tk.LEFT, padx=6)
    ttk.Button(btn_frame, text="Refresh Devices", command=update_devices).pack(side=tk.LEFT, padx=6)

    devices_label = ttk.Label(root, text="Input devices:")
    devices_label.pack(anchor="w", padx=12)
    devices_box = tk.Text(root, height=6, width=60)
    devices_box.pack(fill=tk.BOTH, expand=False, padx=12)

    key_frame = ttk.Frame(root)
    key_frame.pack(fill=tk.X, padx=12, pady=8)
    ttk.Label(key_frame, text="Key test:").pack(side=tk.LEFT)
    ttk.Label(key_frame, textvariable=key_test_status_var).pack(side=tk.LEFT, padx=6)
    ttk.Button(key_frame, text="Start", command=start_key_test).pack(side=tk.LEFT, padx=6)
    ttk.Button(key_frame, text="Stop", command=stop_key_test).pack(side=tk.LEFT)

    device_choice = ttk.Combobox(root, state="readonly", width=60)
    device_choice.pack(fill=tk.X, padx=12, pady=4)

    key_info_frame = ttk.Frame(root)
    key_info_frame.pack(fill=tk.X, padx=12, pady=4)
    ttk.Label(key_info_frame, text="Device:").pack(side=tk.LEFT)
    ttk.Label(key_info_frame, textvariable=key_test_device_var, wraplength=420, justify="left").pack(
        side=tk.LEFT, padx=6
    )

    key_last_frame = ttk.Frame(root)
    key_last_frame.pack(fill=tk.X, padx=12, pady=4)
    ttk.Label(key_last_frame, text="Last key:").pack(side=tk.LEFT)
    ttk.Label(key_last_frame, textvariable=key_test_last_var, wraplength=420, justify="left").pack(
        side=tk.LEFT, padx=6
    )

    config_frame = ttk.Frame(root)
    config_frame.pack(fill=tk.X, padx=12, pady=8)
    config_path_var = tk.StringVar(value=CONFIG_PATH)
    ttk.Label(config_frame, text="Config:").pack(side=tk.LEFT)
    ttk.Entry(config_frame, textvariable=config_path_var, width=50, state="readonly").pack(side=tk.LEFT, padx=6)

    update_devices()
    choices = ["auto"]
    for path, name in list_key_devices():
        choices.append(f"{path} - {name}")
    device_choice["values"] = choices
    device_choice.set(choices[0] if choices else "auto")
    process_messages()

    def on_close() -> None:
        stop_event.set()
        key_test_stop.set()
        root.destroy()

    root.protocol("WM_DELETE_WINDOW", on_close)
    root.mainloop()
