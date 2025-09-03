import os
import sys
import time
from itertools import islice
from pathlib import Path as Pt


def count_nonempty_lines(path: str) -> int:
    """
    Считает непустые строки в файле.
    Поднимает IOError, если файл не найден или недоступен.
    """
    # Читаем весь текст (errors="ignore" — чтобы пропустить некорректные байты)
    text = Pt(path).read_text(encoding="utf-8", errors="ignore")
    # Разбиваем на строки и считаем непустые
    return sum(1 for line in text.splitlines() if line.strip())


def get_line(path: str, line_number: int) -> str:
    """
    Возвращает:
      - при line_number == -1 — последнюю непустую строку файла,
      - при line_number > 0  — указанную строку (1-based).
    В случае отсутствия строки возвращает пустую строку.
    Бросает IOError, если файл недоступен, и ValueError, если line_number < -1 или == 0.
    """
    if line_number < -1 or line_number == 0:
        raise ValueError("line_number должен быть > 0 или == -1")

    # Открываем в текстовом режиме с игнорированием ошибок декодирования
    with open(path, encoding="utf-8", errors="ignore") as f:
        if line_number == -1:
            last = ""
            for line in f:
                s = line.strip()
                if s:
                    last = s
            return last

        # line_number > 0
        # Пропускаем первые line_number-1 строк и берём следующую
        line = next(islice(f, line_number - 1, line_number), "")
        return line.strip()


def write_state_from_last_dir(
    cfg_states_file_path: Pt,
    load_prev_state: int,
    required_files: set[str] = frozenset({"InitSettings.ini", "TimeStates.txt"}),
) -> None:
    """
    Находит среди подпапок parent-папки последнюю (по времени создания) директорию,
    содержащую все файлы из required_files, читает из неё строку load_prev_state
    (1-based; -1 — последняя непустая) из файла cfg_states_file_path.name
    и записывает её в cfg_states_file_path.
    """
    parent = cfg_states_file_path.parent

    # Собираем все «валидные» директории
    valid_dirs = [
        d
        for d in parent.rglob("*")
        if d.is_dir()
        and required_files.issubset({p.name for p in d.iterdir() if p.is_file()})
    ]

    if not valid_dirs:
        return  # Или можно кинуть ошибку, если нужно

    # Сортируем по времени создания и выводим все
    valid_dirs.sort(key=lambda d: d.stat().st_ctime)
    # for d in valid_dirs:
    #     print(f"{d} | created: {d.stat().st_ctime}")

    last_dir = valid_dirs[-1]
    state_filename = cfg_states_file_path.name
    source_state = last_dir / "TimeStates.txt"

    # Получаем нужную строку (1-based; -1 — последняя непустая)
    line = get_line(source_state, load_prev_state)

    # Записываем в целевой файл
    cfg_states_file_path.write_text(line, encoding="utf-8")


def custom_strtobool(val: str) -> bool:
    val_lower = val.strip().lower()
    truthy = {"y", "yes", "t", "true", "on", "1"}
    falsey = {"n", "no", "f", "false", "off", "0"}
    if val_lower in truthy:
        return True
    if val_lower in falsey:
        return False
    raise ValueError(f"Недопустимое булево значение: {val}")


def load_cfg(
    path_to_config: str,
    keys_to_bool: set[str] = None,
    keys_to_int: set[str] = None,
    keys_to_float: set[str] = None,
    keys_to_str: set[str] = None,
    com_line: str = "/////////////////////// | Для коментарів | /////////////////////////",
) -> dict[str, object]:
    keys_to_bool = keys_to_bool or {"Px", "Py", "Pz"}
    keys_to_int = keys_to_int or {
        "Seed",
        *["Sx", "Sy", "Sz"],
        "mode",
        *["AddI", "AddFrom", "RemI", "RemFrom"],
        "LoadPrev",
        *["StepLim", "PrintI", "WriteI"],
    }
    keys_to_float = keys_to_float or {
        "T",
        *["Ax", "Ay", "Az"],
        *["g100", "g010", "g001"],
        *["dg", "C_eq", "C0", "N_tot", "N0_cr", "p_b"],
    }
    keys_to_str = keys_to_str or {"DirPrefix"}

    converters: dict[str, callable] = {}
    converters.update({k: int for k in keys_to_int})
    converters.update({k: float for k in keys_to_float})
    converters.update({k: custom_strtobool for k in keys_to_bool})

    text = Pt(path_to_config).read_text(encoding="utf-8")
    lines = [ln.strip() for ln in text.splitlines()]

    try:
        split_idx = lines.index(com_line)
    except ValueError:
        split_idx = len(lines)

    cfg: dict[str, object] = {}
    for raw in lines[:split_idx]:
        if not raw or raw.startswith("#") or raw.startswith("; "):
            continue
        if ":" not in raw:
            raise ValueError(f"Неправильный формат строки: '{raw}'")
        key, val = (part.strip() for part in raw.split(":", 1))

        if key in converters:
            try:
                cfg[key] = converters[key](val)
            except Exception as e:
                raise ValueError(f"Ошибка конвертации ключа '{key}': {e}")
        else:
            if (
                key in keys_to_str
                and len(val) >= 2
                and val[0] == val[-1]
                and val[0] in {"'", '"'}
            ):
                val = val[1:-1]
            cfg[key] = val

    return cfg
