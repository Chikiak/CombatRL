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

## ISSUE-006: Encapsulated Seeded PRNG and Deterministic RNG Manager

### D11 — Módulo único `src/prng.rs` (no sub-módulos)
**Decisión:** Implementar `PrngStatePOD`, `PRNGError`, `DeterministicRng` y los tests en un único `src/prng.rs`, en contraste con `src/config/` que se dividió en submódulos.

**Contexto:** El issue especifica el path literal `src/prng.rs`. A diferencia de config (que agrupa DTO + POD + loader + vector + error para reuso transversal), el dominio PRNG es cohesivo y pequeño (~130 líneas de lógica). ISSUE-007 importará `PrngStatePOD` — ya es público dentro de `lib.rs` vía `pub mod prng;`.

**Pros:**
- Coherencia con el path literal del issue.
- Sin acoplamiento adicional; el único símbolo reutilizable (`PrngStatePOD`) se exporta directamente.

**Contras:**
- El archivo mezcla POD, error, lógica y tests; aceptable por su tamaño contenido.

---

### D12 — `fork()` = seed derivado + stream incrementado
**Decisión:** `fork()` extrae `child_seed = parent.inner.next_u64()` (avanza el cursor del padre) y construye el hijo con `stream_id = parent.stream_id + 1`.

**Contexto:** El issue exige avanzar el cursor del padre antes de derivar el seed y garantizar streams no solapados. Combinando un seed nuevo (derivado de la salida del padre) con un `stream_id` distinto se obtiene doble garantía de independencia sin depender de la monotonicidad interna de un solo mecanismo.

**Pros:**
- Doble aislamiento: seed criptográficamente distinto + stream distinto.
- `PrngStatePOD.stream_id` queda con valor semántico real, distinguible entre padre e hijos.
- Sencillo de razonar y de testear (`test_fork_stream_hierarchy`).

**Contras:**
- Consume un `next_u64()` extra del padre (desplaza la secuencia una palabra); es intencional y determinista.

---

### D13 — `gen_range_f32` sin división (inyección de mantisa)
**Decisión:** Tomar `next_u32()`, descartar los 9 bits bajos (`>> 9`) para quedarse con 23 bits de mantisa, inyectarlos en `f32::from_bits(0x3F80_0000 | mantissa)` para obtener `[1.0, 2.0)`, restar `1.0` → `[0.0, 1.0)`, y escalar `low + unit * (high - low)`.

**Contexto:** El issue prohíbe división de punto flotante y FMA en el muestreo. Este mapeo usa solo OR-por-bits, resta y multiplicación, cumpliendo la invariante y siendo alineable a SIMD.

**Pros:**
- Cumple estrictamente la invariante "Division-Free IEEE 754 Mantissa Bit Injection".
- Sin FMA ni división: resultados bit-exactos cross-platform.

**Contras:**
- Asume `low <= high` (contract del llamador); el resultado respeta `[low, high)`.

---

### D14 — Serde del PRNG delegado a `PrngStatePOD`
**Decisión:** `DeterministicRng` no deriva `Serialize`/`Deserialize`; se implementan manualmente delegando en `export_pod()` / `from_pod()`. `PrngStatePOD` deriva los traits (el campo `u128` es serializable por serde).

**Contexto:** `ChaCha8Rng` no implementa los traits serde (motivo de `PrngStatePOD`). Re-hidratar reconstruyendo desde `seed` + `set_stream(stream_id)` + `set_word_pos(word_pos)` restaura el cursor exacto.

**Pros:**
- Formato de estado explícito y estable (`seed`, `stream_id`, `word_pos`), válido como payload de checkpoint.
- `set_word_pos(u128)` no trunca el offset interno de ChaCha.

**Contras:**
- Serializa el objeto completo como POD; cualquier cambio futuro en la representación interna debe reflejarse en `from_pod`.

---

## ISSUE-007: Core Simulation State Representation and Deterministic State Hashing

### D15 — Estructura y alineación SIMD en `FighterState` y `WorldState`
**Decisión:** `FighterState` declarado como `repr(C, align(32))` de 32 bytes exactos; `WorldState` como `repr(C, align(64))` de 2112 bytes exactos (33 bloques L1 de 64 bytes, sin padding final), garantizando localidad de caché y cero padding no inicializado.

**Contexto:** Cumple los requisitos de L1 cache line alignment (64 bytes) y SIMD alignment (32 bytes) para entidades, evitando divisiones de caché y asegurando que las ranuras inactivas `[active_count..64]` estén completamente zero-inicializadas (`0x00`).

**Pros:**
- Layout gapless validado en compile-time con `static_assertions`.
- Cero asignaciones de heap (`Copy`/`Clone`/`reset`/`compute_hash`).

**Contras:**
- Requiere mantenimiento estricto del orden y padding explícito ante cambios de campos.

---

### D16 — Hash determinista canónico `xxh3_64` vía crate `xxhash-rust`
**Decisión:** Utilizar el crate canónico `xxhash-rust` (`features = ["xxh3"]`) para invocar `xxhash_rust::xxh3::xxh3_64(&[u8])` sobre la vista binaria de `WorldState`, descartando implementaciones caseras anteriores para garantizar interoperabilidad exacta con `xxhash.xxh3_64` de Python y lectura little-endian robusta ante cambios de endianness de arquitectura (NFR-02).

**Contexto:** La auditoría detectó que un hash casero anterior no cumplía con el estándar XXH3-64 ni manejaba endianness canónico. `xxhash-rust` provee un hash `no_std`, zero-alloc y conforme al estándar oficial.

**Pros:**
- Interoperabilidad bit-exacta con bibliotecas de Python y otros runtimes (`xxhash`).
- Little-endian canónico garantizado cross-platform.
- Cero asignaciones de heap (single-pass sobre `&[u8]`).

**Contras:**
- Dependencia externa en Cargo.toml (mitigada por ser `#![no_std]` y puro Rust).

---

### D17 — Canonicalización branchless de ceros negativos (`-0.0f32` → `+0.0f32`)
**Decisión:** Manipulación a nivel de bits de IEEE 754 (`to_bits`, aislamiento de signo y magnitud cero) ejecutada en tiempo de mutación/canonicalización sin saltos condicionales (`branchless`), asegurando invariabilidad de hash ante variaciones de signo en ceros.

**Contexto:** Evita discrepancias de hash `xxh3_64` causadas por la representación de `-0.0f32` (`0x80000000`) frente a `+0.0f32` (`0x00000000`).

**Pros:**
- Cero saltos condicionales (branchless), optimizable por LLVM.
- Preserva valores distintos de cero intactos.

**Contras:**
- Lógica de bits explícita que requiere comentarios sobre semántica IEEE 754.

---

### Resumen de dependencias nuevas (relativas a Phase 0)
- `thiserror` (dependencia de runtime) — manejo idiomático de errores.
- `static_assertions` (dev-dependency) — aserciones de compile-time para tamaño y alineación en PODs.
- `xxhash-rust` (dependencia de runtime) — cálculo hash xxh3_64 para determinismo y state hashing.
- `rand` / `rand_chacha` (dependencia de runtime) — generadores de números pseudo-aleatorios criptográficos para el PRNG determinista.
- `alloc_counter` (dev-dependency) — verificación de cero alocaciones heap en tests unitarios (`assert_no_alloc`).

---

## ISSUE-008: [Engine Core] Fixed-Rate Discrete Tick Engine Loop and State Advance Mechanics

### D18 — Fase 0 de validación fail-fast previa a mutación en `step()`
**Decisión:** Introducir un pase inicial de validación ("Phase 0") en el método `step()` de `TickEngine` que verifica antes de cualquier modificación de estado que los `entity_id` de las acciones correspondan exactamente al índice del slot (`act_id as usize == i`) y estén dentro del rango de entidades activas (`< active_count`). Si falla, retorna `Err(EngineError::InvalidEntityId(act_id))` sin alterar el estado ni avanzar el tick.

**Contexto:** El plan inicial especificaba control de errores ante IDs inválidos, pero añadir esta validación como pase estricto previo evita mutaciones parciales ante acciones corruptas o desincronizadas, garantizando robustez transaccional por tick.

**Pros:**
- Transaccionalidad estricta: si una acción es inválida, el mundo permanece intacto (`current_tick` no avanza).
- Fail-fast determinista y predecible.

**Contras:**
- Añade un bucle de validación lineal O(N) adicional antes del pipeline cinemático (despreciable con N ≤ 64).

---

### D19 — Normalización de intención con Newton-Raphson rsqrt
**Decisión:** En la Fase 1 del pipeline de `step()`, en lugar de utilizar división de punto flotante convencional para normalizar el vector de movimiento `move_intent`, se emplea una aproximación de raíz cuadrada inversa rápida mediante el algoritmo de Newton-Raphson (`deterministic_rsqrt_nr`) con constante Quake (`0x5F37_59DF`) para vectores con norma al cuadrado mayor a 1.0.

**Contexto:** Las instrucciones de división (`div`) y raíz cuadrada (`sqrt`) estándar de punto flotante pueden diferir ligeramente en microarquitecturas x86_64 frente a ARM64, rompiendo la garantía bit-exacta de determinismo (NFR-02). Una implementación basada en operaciones enteras de bits (`to_bits`, `from_bits`) garantiza idénticos resultados en cualquier plataforma.

**Pros:**
- Determinismo bit-exacto cross-platform garantizado (sin dependencias de FPU divergentes para divisiones/raíces).
- Cero divisiones de punto flotante en el trayecto crítico de normalización.

**Contras:**
- Lógica numérica ligeramente más compleja que requiere validación de precisión (suficiente para normalización direccional de intendencia).

---

### D20 — Integración cinemática directa (velocidad objetivo sin aceleración)
**Decisión:** En la Fase 2 del pipeline, el cálculo de la velocidad en el siguiente tick establece directamente la velocidad objetivo (`target_v = dir * move_speed`) aplicando Flush-To-Zero (`ftz_zero`), sin acumulación incremental de aceleración (`v_{t+1} = v_t + a·Δt`). La posición se integra mediante Euler estándar (`p_{t+1} = p_t + v_{t+1} · Δt`).

**Contexto:** El sistema de aceleración y física de combate propiamente dicha se delegó a la Phase 2. Para la Phase 1 (enfoque exclusivamente cinemático y de límites), asignar velocidad objetivo directa evita drift de velocidad innecesario y simplifica la verificación de spacing.

**Pros:**
- Modelo cinemático sencillo, determinista y predecible para pruebas de espaciamiento y colisión inicial.
- Facilita la certificación de throughput > 1,000,000 SPS.

**Contras:**
- Ausencia temporal de inercia o físicas de aceleración (se introducirán en Phase 2).

---

### D21 — FTZ y clamping de velocidad por manipulación bitwise branchless
**Decisión:** La supresión de subnormales (`ftz_zero`) y el reajuste de velocidad en colisiones con paredes (Fase 3) se implementan mediante operaciones a nivel de bits (máscaras y `wrapping_neg`) sin saltos condicionales (`branchless`). En la colisión de eje, si se recorta la posición, la componente de velocidad correspondiente se anula bit a bit forzando `+0.0f32` (`0x00000000`).

**Contexto:** Previene microcode stalls por números subnormales y evita contaminación de signos en ceros ante colisiones con los límites del arena.

**Pros:**
- Cero saltos condicionales (branchless), eliminando penalizaciones por fallos de predicción de saltos.
- Forzado robusto a `+0.0f32` canónico, evitando la aparición de `-0.0` en rebotes o paradas.

**Contras:**
- Código que depende de manipulación explícita de representación binaria IEEE 754.

---

### D22 — API extendida del motor (`spawn`, `state_mut`, `reset` reconstructivo)
**Decisión:** Proveer métodos públicos adicionales en `TickEngine`: `spawn()` para añadir entidades con asignación automática de atributos base y posicionamiento simétrico inicial (Fighter 0 a la izquierda, Fighter 1 a la derecha con orientaciones opuestas), `state_mut()` para mutaciones controladas en tests, y `reset(new_seed)` implementado mediante reconstrucción limpia (`*self = Self::new(...)`). El enum `EngineError` incorpora la variante `MaxEntitiesReached`.

**Contexto:** Facilita la configuración headless, la inicialización de escenarios de prueba deterministas y la gestión de re-hacer estados en bucles de entrenamiento MARL.

**Pros:**
- Inicialización y reinicio limpios sin fugas de estado residual en memoria.
- Soporte robusto para la capacidad máxima de entidades (`MAX_SIMULTANEOUS_ENTITIES = 64`).

**Contras:**
- `reset` reasigna el struct completo clonando/recreando arena y atributos (operación ligera en stack, aceptable).

---

## ISSUE-009: [Engine Core] Bit-Exact Determinism Verification and Multi-Instance Regression Test Suite

### D23 — Configuración simplificada de compilador en `.cargo/config.toml`
**Decisión:** Configurar los perfiles de compilación y flags en `.cargo/config.toml` habilitando `lto = "fat"` y `codegen-units = 1` en `[profile.release]`, y fijando `rustflags = ["-C", "target-feature=-fma"]` a nivel global de compilación.

**Contexto:** El plan original contemplaba deshabilitar la reducción FMA mediante argumentos LLVM (`llvm-args=-enable-fma-lower=false`), pero dicho parámetro fue omitido por incompatibilidades con versiones modernas de LLVM, siendo suficiente la desactivación de la característica de objetivo `target-feature=-fma` para evitar variaciones de redondeo FMA entre arquitecturas.

**Pros:**
- Compatibilidad cross-version con toolchains modernos de Rust y LLVM.
- Desactivación efectiva de FMA para garantizar determinismo cross-platform.

**Contras:**
- Menor optimización de fused multiply-add en hardware que lo soporte nativamente (aceptado como costo necesario para estricto determinismo NFR-02).

---

### D24 — Harness de benchmarking personalizado (`harness = false`) con `black_box`
**Decisión:** Implementar el benchmark de rendimiento (`benches/bench_engine.rs`) como un binario independiente (`harness = false`) utilizando `std::time::Instant`, bucles con `core::hint::black_box` tanto en las acciones de entrada como en los hashes de salida, y acumulación XOR (`acc`) para impedir la eliminación de código muerto (DCE) por parte del optimizador de LLVM.

**Contexto:** Permite medir con precisión milisegundo el throughput real de pasos de simulación sin depender de frameworks de benchmarking externos complejos, garantizando que el compilador no optimice el bucle de ejecución.

**Pros:**
- Control absoluto sobre la medición de pasos por segundo (SPS).
- Prevención robusta de DCE mediante barreras `black_box`.

**Contras:**
- Requiere lógica propia de cálculo de SPS en lugar de estadísticas automáticas de un framework especializado.

---

### D25 — Certificación de throughput condicional mediante `BENCH_STRICT_CERTIFY`
**Decisión:** El benchmark emite por defecto las métricas de pasos por segundo (`Steps Per Second: ...`), activando la aserción estricta de superación de 1,000,000 SPS únicamente cuando la variable de entorno `BENCH_STRICT_CERTIFY` está presente.

**Contexto:** El rendimiento por segundo depende fuertemente de las capacidades del hardware del host donde se ejecute la compilación (CI vs máquina de desarrollo local). Hacer la aserción condicional evita falsos positivos en entornos de desarrollo de menor potencia sin perder la capacidad de validación estricta en pipelines de CI certificados.

**Pros:**
- Evita fallos de test en entornos locales con hardware limitado.
- Permite certificación formal estricta en servidores CI de alto rendimiento.

**Contras:**
- Requiere configurar la variable de entorno en scripts de CI para forzar la validación de rendimiento.

---

### D26 — Pruebas de integración de determinismo y aislamiento de `reset()`
**Decisión:** Implementar la suite de integración en `tests/test_determinism.rs` ejecutando dos instancias paralelas (`engine_a`, `engine_b`) durante 10,000 ticks con un flujo de acciones estocástico derivado de un stream PRNG aislado, verificando igualdad exacta de hashes `xxh3_64` en cada tick (con volcado detallado de diagnóstico ante discrepancias) y validando mediante `from_raw_parts` que `reset()` restaura un estado prístino con ceros absolutos (`0x00`) en todas las ranuras inactivas.

**Contexto:** Valida formalmente el hito final de Phase 1, garantizando que no existen fugas de memoria residual, derivas de coma flotante ni divergencias de estado a lo largo de ejecuciones prolongadas.

**Pros:**
- Cobertura de prueba exhaustiva (10,000 ticks continuos).
- Diagnóstico exacto (tick y slot) ante cualquier mínima divergencia de estado.
- Verificación estricta de limpieza de memoria en slots inactivos.

**Contras:**
- Tiempo de ejecución ligeramente mayor en la suite de integración de tests (del orden de milisegundos, aceptable).