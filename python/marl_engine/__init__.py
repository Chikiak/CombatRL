"""
MARL Engine Native Extension Wrapper
"""

try:
    from marl_engine._marl_engine_native import *
    from marl_engine._marl_engine_native import __doc__ as _native_doc
except ImportError as e:
    raise ImportError(
        "No se pudo cargar el módulo nativo Rust '_marl_engine_native'. "
        "Asegúrate de haber ejecutado 'maturin develop --no-default-features --features python'."
    ) from e