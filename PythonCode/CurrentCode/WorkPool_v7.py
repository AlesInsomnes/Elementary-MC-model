import sys
import time
from itertools import islice
from pathlib import Path as Pt
from shutil import copy2
from subprocess import CREATE_NEW_CONSOLE, Popen

from utils import count_nonempty_lines, load_cfg, write_state_from_last_dir


def get_main_dir() -> Pt:
    """Где лежит скрипт (или exe-директория при frozen)."""
    if getattr(sys, "frozen", False):
        return Pt(sys.executable).parent
    return Pt(__file__).resolve().parent


def make_output_dir(base: Pt, name: str) -> Pt:
    """Создаёт вложенную папку с меткой времени."""
    stamp = time.strftime("%Y%m%dT%H%M%S") + f"_{int(time.time_ns() % 1_000_000)}"[:-2]
    out = (base / name).resolve() / stamp
    out.mkdir(parents=True, exist_ok=True)
    return out


def ensure_files_exist(cfg: dict, keys: list[str], base: Pt, dest: Pt):
    """Копирует в dest все пути из cfg[keys] (абсолютные или относительно base)."""
    for key in keys:
        src = Pt(cfg[key])
        src = src if src.is_absolute() else (base / src)
        if not src.exists():
            raise FileNotFoundError(f"{key}: файл не найден: {src}")
        dst = dest / src.name
        copy2(src, dst)
        if not dst.exists():
            raise IOError(f"{key}: не удалось скопировать в {dst}")
        cfg[key] = dst


def run(cfg: dict):
    """Запускает все симуляции параллельно, ограничивая по NumProcesses."""
    out_dir = cfg["OutputDirectory"]
    tpl_text = cfg["CfgTemplatePath"].read_text(encoding="utf-8")
    total = count_nonempty_lines(cfg["CombinationsFilePath"])
    procs = []

    with cfg["CombinationsFilePath"].open(encoding="utf-8", errors="ignore") as f:
        for idx, raw in enumerate(filter(str.strip, f), 1):
            params = raw.strip().split(" +| ")
            params = [p.replace("\\n\\", "\n") for p in params]

            try:
                cfg_text = tpl_text.format(*params).strip()
            except IndexError:
                print(f"[WARN] Недостаточно параметров в строке {idx}", file=sys.stderr)
                continue

            # Сохраняем конфиг
            (out_dir / cfg["CfgFileName"]).write_text(cfg_text, encoding="utf-8")

            # Восстановление состояния (если нужно)
            try:
                load_state = int(params[26])
            except (IndexError, ValueError):
                load_state = 0
            if load_state != 0:
                if cfg["loadPrevMode"] == "Last":
                    write_state_from_last_dir(
                        out_dir / cfg["cfgStatesFileName"],
                        load_state,
                        {"InitSettings.ini", "TimeStates.txt"},
                    )

            # Запуск процесса
            procs.append(
                Popen(
                    [str(cfg["ExecutablePath"])],
                    cwd=out_dir,
                    creationflags=CREATE_NEW_CONSOLE,
                )
            )

            # Ограничение по количеству параллельных
            while len(procs) >= cfg["NumProcesses"]:
                time.sleep(1)
                procs = [p for p in procs if p.poll() is None]
                print(f"Active: {len(procs)}/{total - idx + 1}")

            time.sleep(0.25)

    # Ждём всех
    while procs:
        time.sleep(1)
        procs = [p for p in procs if p.poll() is None]
        print(f"Active: {len(procs)} — waiting...")

    print("Все процессы завершены.")


def main():
    try:
        base = get_main_dir()
        print(f"Main dir: {base}")

        cfg = load_cfg(
            base / "WorkCfg.ini",
            keys_to_int={"NumProcesses"},
            keys_to_str={
                "OutputDirectory",
                "StatesFileName",
                "cfgStatesFileName",
                "CfgFileName",
                "loadPrevMode",
                "CfgTemplatePath",
                "CombinationsFilePath",
                "ExecutablePath",
            },
            com_line="///////////////////////// | Для коментарів | /////////////////////////",
        )

        # Проверяем обязательные ключи и тип NumProcesses
        req = {
            "OutputDirectory",
            "StatesFileName",
            "cfgStatesFileName",
            "CfgFileName",
            "loadPrevMode",
            "CfgTemplatePath",
            "CombinationsFilePath",
            "ExecutablePath",
            "NumProcesses",
        }
        missing = req - cfg.keys()
        if missing:
            raise KeyError(f"Отсутствуют ключи: {missing}")

        cfg["NumProcesses"] = int(cfg["NumProcesses"])
        if cfg["NumProcesses"] <= 0:
            raise ValueError("NumProcesses должен быть > 0")

        # Готовим папку
        out_dir = make_output_dir(base, cfg["OutputDirectory"])
        print(f"Output dir: {out_dir}")
        cfg["OutputDirectory"] = out_dir

        # Копируем шаблоны/файлы
        ensure_files_exist(
            cfg,
            [
                "CfgTemplatePath",
                "CombinationsFilePath",
                "ExecutablePath",
                "CfgFileName",
            ],
            base,
            out_dir,
        )

        if cfg["loadPrevMode"] != "None":
            ensure_files_exist(
                cfg,
                [
                    "cfgStatesFileName",
                ],
                base,
                out_dir,
            )

        run(cfg)

    except Exception as e:
        print(f"[ERROR] {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
