import os
import sys
from pathlib import Path as Pt
from timeit import timeit
from typing import Any, Dict, Iterator, List, Optional, Sequence, Set, Tuple, Union

import numpy as np


def read_large_file_lines(file_path: Pt) -> Iterator[str]:
    """
    Генератор для построчного чтения очень больших файлов.
    Останавливается при первой пустой строке, пропуская её.
    """
    with file_path.open("r", encoding="utf-8", errors="ignore") as f:
        for raw in f:
            line = raw.strip()
            if not line:
                break
            yield line


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
        *["AddI", "AddFrom", "RemI", "RemFrom"],
        *["LoadPrev", "LoadOption"],
        *["StepLim", "PrintI", "WriteI"],
    }
    keys_to_float = keys_to_float or {
        "T",
        *["Ax", "Ay", "Az"],
        *["g100", "g010", "g001"],
        "mode",
        *["dg", "C_eq", "C0", "N_tot", "N0_cr", "p_b", "p_pow"],
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


def gen_xyz(
    size_x: int = 10, size_y: int = 10, size_z: int = 10, mode: int = 1
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """
    Генерирует координаты (X, Y, Z) внутри объёма size_x×size_y×size_z
    с маской в зависимости от mode:

      1 — шахматка: (X+Y+Z)%2==0
      2 — все точки
    """
    # Проверка входных данных
    if any(d <= 0 or not isinstance(d, int) for d in (size_x, size_y, size_z)):
        raise ValueError("Размеры должны быть положительными целыми числами")

    # Строим координатную сетку
    X, Y, Z = np.indices((size_x, size_y, size_z))

    # Выбираем маску
    if mode == 1:
        mask = (X + Y + Z) % 2 == 0
    elif mode == 2:
        mask = np.ones_like(X, dtype=bool)
    else:
        raise ValueError(f"Недопустимый mode: {mode}. Ожидалось 1 или 2.")

    # Возвращаем только отмаскированные точки (они уже в 1D)
    return X[mask], Y[mask], Z[mask]


def prep_cuboid(
    state: np.ndarray,
    X: np.ndarray,
    Y: np.ndarray,
    Z: np.ndarray,
    start: tuple[float, float, float] = (0.0, 0.0, 0.0),
    size: tuple[float, float, float] = (1.0, 1.0, 1.0),
    value: int = 1,
) -> np.ndarray:
    """
    Задаёт внутри векторизованной 3D‑сетки (X, Y, Z) кубоид,
    помечая его площадь значением `value` в массиве `state`.

    Args:
        state: 1D numpy‑массив длинны N = X.size = Y.size = Z.size.
        X, Y, Z: 1D numpy‑массивы координат той же длины N.
        start: координаты (x0, y0, z0) нижнего левого угла кубоида.
        size: размеры (sx, sy, sz) по осям; ожидаются положительные.
        value: значение, которым заполняется кубоид.

    Returns:
        Этот же массив `state`, но с `state==value` внутри заданного кубоида.
    """
    # Быстрая проверка согласованности входных данных
    if state.ndim != 1 or X.ndim != 1 or Y.ndim != 1 or Z.ndim != 1:
        raise ValueError(
            "state, X, Y и Z должны быть одномерными массивами одинаковой длины"
        )
    if state.size != X.size or X.size != Y.size or Y.size != Z.size:
        raise ValueError("Размеры X, Y, Z и state должны совпадать")
    if any(s <= 0 for s in size):
        raise ValueError("Все размеры кубоида должны быть положительными")

    x0, y0, z0 = start
    sx, sy, sz = size

    # Создаём булеву маску кубоида одним выражением
    mask = (
        ((X >= x0) & (X < x0 + sx))
        & ((Y >= y0) & (Y < y0 + sy))
        & ((Z >= z0) & (Z < z0 + sz))
    )

    # Заполняем кубоид
    state[mask] = value
    return state


class Template:
    """
    Управляет параметрами и генерирует:
      - компактную строку параметров через splitter
      - полный текст по шаблону из файла
    """

    DEFAULT_ITEMS: Dict[str, Dict[str, Any]] = {
        "test_bool": {"v": False, "f": "{}"},
        "test_int": {"v": 1345, "f": "{}"},
        "test_float": {"v": 9.58767e-08, "f": "{:.5e}"},
        "test_str": {"v": "some str", "f": "{}"},
    }

    def __init__(
        self,
        items: Dict[str, Dict[str, Any]] = None,
        template_path: Union[Pt, str] = "_TestTemplate.txt",
    ):
        # Копируем базовый словарь, чтобы не менять класс-атрибут
        base = items or self.DEFAULT_ITEMS
        self.items: Dict[str, Dict[str, Any]] = {k: v.copy() for k, v in base.items()}
        # Порядок ключей
        self.order = list(self.items.keys())
        # Читаем шаблон один раз
        self.template_str = Pt(template_path).read_text(encoding="utf-8")

    def set_item_v(self, key: str, value: Any) -> None:
        """Устанавливает новое значение для существующего параметра"""
        if key in self.items:
            self.items[key]["v"] = value
        else:
            raise KeyError(f"Параметр '{key}' не найден")

    def set_item_f(self, key: str, format_string: str) -> None:
        """Устанавливает новую строку формата для существующего параметра"""
        if key in self.items:
            self.items[key]["f"] = format_string
        else:
            raise KeyError(f"Параметр '{key}' не найден")

    def _formatted_values(self) -> list[str]:
        """
        Форматирует все значения в порядке self.order;
        булевы значения приводятся к нижнему регистру.
        """
        formatted = []
        for name in self.order:
            data = self.items[name]
            s = data["f"].format(data["v"])
            formatted.append(s.lower() if isinstance(data["v"], bool) else s)
        return formatted

    def template_compact(self, splitter: str = " +| ") -> str:
        """Возвращает компактную строку значений через splitter"""
        return splitter.join(self._formatted_values())

    def template_full(self) -> str:
        """Возвращает полный текст шаблона с подстановкой всех значений"""
        return self.template_str.format(*self._formatted_values())


# Пример использования
if __name__ == "__main__":
    tpl = Template()
    tpl.set_item_v("test_int", 2025)
    tpl.set_item_f("test_bool", "{!s}")
    print(f"\ntpl items:\n{tpl.items}")
    print(f"\nData (Compact):\n{tpl.template_compact()}")
    print(f"\nData (Full):\n{tpl.template_full()}")
