# Phase 1 — Technical Decisions Log

Este documento registra las decisiones técnicas tomadas durante la implementación de los issues de `docs/issues/phase1.md`. Cada decisión incluye contexto, opción elegida, justificación y Pros/Contras considerados.

---

## ISSUE-005: Data-Driven Arena and Fighter Attribute Configuration Schemas

### D1 — Organización del código interno de configuración
**Decisión:** Sub-módulos bajo `src/config/` (`mod.rs`, `vector.rs`, `error.rs`, `dto.rs`, `pod.rs`, `loader.rs`).

**Contexto:** El issue especifica el path literal `src/config.rs`, pero ISSUE-007 reutilizará `Vector2D` y `ConfigError` desde el módulo de estado. Una estructura modular facilita esos imports sin acoplar todo en un único archivo largo (~400-500 líneas con DTO + POD + Vector2D + Error + loader + tests).

**Pros:**
- Separación clara de responsabilidades (vector / error / dto / pod / loader).
- Permite que ISSUE-007 importe limpiamente `Vector2D` y `ConfigError` sin traer todo el dominio de configuración.
- Escalable para añadir schemas adicionales (armas, escenarios) en futuros issues.

**Contras:**
- Diverge del path literal `src/config.rs` del issue (usamos `src/config/mod.rs`). Aceptamos la desviación porque `mod config;` sigue funcionando igual desde `lib.rs`.
- Más archivos para una fase inicial pequeña; justificado por la reutilización transversal.

---

### D2 — Carga de archivos JSON
**Decisión:** Runtime vía `std::fs` con paths relativos al directorio del crate (`env!("CARGO_MANIFEST_DIR")` como base para los defaults).

**Contexto:** Cumple NFR-04 (Data-Driven Design), permitiendo modificar `assets/configs/*.json` sin recompilar.

**Pros:**
- Cumple estrictamente NFR-04: datos externos, sin recompilación.
- Sencillo, usa primitivas estándar de la stdlib.
- `ConfigError::FileNotFound` se materializa naturalmente cuando el archivo no existe.

**Contras:**
- Depende del CWD o de `CARGO_MANIFEST_DIR`. Para tests usaremos `env!("CARGO_MANIFEST_DIR")` concatenado con `assets/configs/...` para evitar dependencias del directorio de ejecución.
- Posible fallo en runtime si los archivos no se distribuyen con el binario (aceptado: el deploy debe empacar la carpeta `assets/`).

---

### D3 — Enum `ConfigError`
**Decisión:** Implementar `ConfigError` con la crate `thiserror`.

**Contexto:** El issue define cuatro variantes: `FileNotFound`, `InvalidSchema`, `InvalidBounds { parameter: &'static str, value: f32 }`, `SubnormalFloatDetected { parameter: &'static str }`.

**Pros:**
- Boilerplate mínimo: `#[derive(Debug, Error)]` + `#[error("...")]` genera `Display` e impls de conversión automáticas.
- Mensajes de error consistentes y descriptivos.
- Integración limpia con el operador `?` en cadenas de carga/parsing.

**Contras:**
- Añade una dependencia más al `Cargo.toml`. Aceptada: `thiserror` es de hecho estándar en el ecosistema Rust (cero costo en runtime, solo macros de derivación).

---

### D4 — Detección de NaN / Infinitos / Subnormales
**Decisión:** Usar primitivas de la stdlib (`f32::is_finite` + `f32::is_normal` para valores no nulos).

**Contexto:** El issue exige rechazar `NaN`, `±Infinity` y subnormales (`|x| < 1.17549435e-38`), y además forzar floats normales ($|x| \ge 1.17549435 \times 10^{-38}$) para valores distintos de cero. `is_normal()` devuelve `false` para cero, por lo que la validación será: aceptar `0.0` y `is_normal()`, rechazar el resto de no-finitos/ subnormales.

**Pros:**
- Código legible e idiomático, optimizado por LLVM.
- `is_normal()` cubre NaN/Inf/subnormales de un golpe para valores distintos de 0.
- Fácil de mantener por colaboradores futuros.

**Contras:**
- Requiere combinarse con un check de cero explícito (aceptamos `0.0` como válido allí donde los bounds superficiales lo permitan, p.ej. `move_speed >= 0.0`).
- No distingue clase exacta (signaling NaN vs quiet) — no es necesaria para este issue.

---

### D5 — ID de string mayor a 32 bytes
**Decisión:** Rechazar con `ConfigError::InvalidSchema` cuando el ID exceda los 32 bytes disponibles en `[u8; 32]`.

**Contexto:** El layout `FighterAttributesPOD` y `ArenaConfigPOD` reserva exactamente 32 bytes para el `id`. Una truncación silente introduciría colisiones potenciales.

**Pros:**
- Fail-fast explícito: evita colisiones de ID por truncado silencioso.
- Coherente con la filosofía de validación estricta (strict validation contracts) que el issue defiende.

**Contras:**
- Limita IDs a 31 bytes útiles (reservamos el último como terminador nulo `0x00`). El padding zero-padded cubre todos los bytes no usados, así que técnicamente se admiten 32 bytes si se rellena todo el array con el string sin terminador; la convención final será: usar toda la longitud ≤ 32, rellenar con `0x00`, y rechazar si `> 32`.
- En este issue no se usa el ID más allá de identificación de perfil, así que la limitación es aceptable.

---

### D6 — Aserciones de layout (size/align) en compile-time
**Decisión:** Usar la crate `static_assertions` con `assert_eq_size!` y `assert_eq_align!` en cada POD.

**Contexto:** El issue exige `ArenaConfigPOD` y `FighterAttributesPOD` con tamaño y alineación exactos de 64 bytes; `Vector2D` con 8 bytes/align 8. Fallar la compilación ante un cambio de layout es más seguro que detectarlo solo en tests.

**Pros:**
- Errores de layout se capturan en `cargo check` (antes de correr tests).
- Macros legibles y específicas para size/align (`assert_eq_size!`, `assert_eq_align!`).
- Crate estándar bien establecida en el ecosistema.

**Contras:**
- Añade 1 `dev-dependency` pequeña (solo macros de compile-time, cero costo en runtime). Justificada.
- Sintaxis adicional que conviene documentar brevemente para futuros contribuidores.

---

### D7 — Spawn points concretos del arena
**Decisión:** Manejar en ISSUE-007/008 (WorldState / TickEngine). `ArenaConfigPOD` almacena solo `spawn_margin`; los spawn points concretos no forman parte del schema de configuración del arena.

---

### D8 — Valores predeterminados y nomenclatura de esquemas JSON
**Decisión:** Utilizar `snake_case` en los campos de los DTOs serde y establecer valores predeterminados orientados a combates competitivos equilibrados (Arena 30×18, spawn_margin 2.0; Luchador con max_health 150.0, max_stamina 60.0, move_speed 5.0, weight 1.0, collider 0.8×1.8).

**Contexto:** Los esquemas deben ser legibles y alineados con las convenciones de Rust (`snake_case`) y proveer métricas espaciales aptas para el entrenamiento de espaciado y timing en MARL.

**Pros:**
- Coherencia directa con los nombres de campos en las estructuras Rust DTO sin requerir mapeos manuales ni renombrados verbosos.
- Valores espaciales equilibrados para permitir la evaluación de spacing y colisiones sin saturar el espacio de estados.

**Contras:**
- No sigue estrictamente `camelCase` de convenciones JavaScript puras, pero se prioriza la idiomaticidad del motor backend en Rust.

---

### D9 — Traits derivados en estructuras POD y API de carga
**Decisión:** Derivar `Copy, Clone, Debug, Default` en las estructuras POD y proveer funciones libres de carga `load_arena` y `load_fighter` con resolución automática basada en `CARGO_MANIFEST_DIR`.

**Contexto:** Permite inicializar arrays de manera limpia con `Default::default()` (útil para rellenar slots inactivos de entidades) y simplifica los tests y la carga headless sin requerir instancias de estado con referencias mutables o gestores de archivos complejos.

**Pros:**
- Cero overhead de indirección y facilidad para crear stubs o valores por defecto.
- Funciones de carga independientes que simplifican el re-uso en futuros issues.

**Contras:**
- El trait `Default` inicializa floats a `0.0` (que no son valores normales válidos si se validan directamente), por lo que las estructuras POD con ceros deben considerarse inválidas hasta ser pobladas por el loader validado.

---

### D10 — Variante `ConfigError::IoError(ErrorKind)`
**Decisión:** Incluir `ConfigError::IoError(std::io::ErrorKind)` como quinta variante en el enum de errores y actualizar el `From<std::io::Error>` para mapear `NotFound` → `FileNotFound` y cualquier otro error de IO → `IoError`.

**Contexto:** El issue original lista 4 variantes base, pero para robustez ante fallos de permisos, bloqueos o errores de sistema no-NotFound al leer ficheros de configuración, disponer de la variante `IoError` evita colapsar todos los fallos IO en una única categoría genérica.

**Pros:**
- Diagnóstico preciso de errores de E/S del sistema operativo más allá de la ausencia de archivo.
- Mapeo limpio e interoperable con `thiserror`.

**Contras:**
- Amplía ligeramente el contrato de errores respecto a la enumeración estricta de 4 variantes del issue. Asumido y justificado por robustez.

---

### Resumen de dependencias nuevas (relativas a Phase 0)
- `thiserror` (dependencia de runtime) — manejo idiomático de errores.
- `static_assertions` (dev-dependency) — aserciones de compile-time para tamaño y alineación en PODs (`ArenaConfigPOD`, `FighterAttributesPOD`, `Vector2D`).
- `alloc_counter` (dev-dependency) — verificación de cero alocaciones heap en tests unitarios (`assert_no_alloc`).



---