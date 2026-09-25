## Issues para la Fase 2: Combat Engine (Frame-Data FSM, Continuous Collisions, and Weapons)

---

### ISSUE-010: [Combat / Weapons] Catálogo Data-Driven de Armas y Layout POD de Atributos Estandarizados

**Metadatos:** Capa: Backend / Core Rust  
**Dependencias Directas:** [ISSUE-005]

#### 1. Propósito y Razonamiento
* **Secuencia:** Constituye el primer paso funcional de la Fase 2. Todo el sistema de combate (FSM de frame-data, cálculo de alcances espaciales y resolución de daños) depende contractualmente de los parámetros temporales y físicos de las armas. Establecer primero este catálogo desacoplado permite alimentar la FSM y los detectores de colisión sin valores embebidos (*hardcoded*).
* **Valor de Negocio:** Satisface directamente los requerimientos **FR-04** (catálogo de al menos 4 armas diferenciadas) y **NFR-04** (diseño Data-Driven). Permite a los diseñadores de juego e investigadores de MARL rebalancear métricas de armamento (duración de fases, costos y daños) de forma externa en archivos JSON sin recompilar el código fuente.
* **Riesgos Técnicos Prevenidos:** Previene bloqueos infinitos de la FSM (animaciones congeladas con 0 frames de duración), microcódigo traps en CPU causados por flotantes subnormales en el daño o knockback, e inconsistencias en las dimensiones de observación para modelos neuronales al proyectar vectores de atributos con longitud variable.
* **Decisiones de Arquitectura:**
  * Desacoplar la ingestión Serde (`WeaponConfigDTO`) del almacenamiento en tiempo de ejecución (`WeaponConfigPOD`).
  * Diseñar `WeaponConfigPOD` con representación fija `repr(C, align(64))` con un tamaño exacto de 64 bytes (una línea de caché L1), asegurando cero indirecciones y compatibilidad con registros SIMD.
  * Proveer un catálogo con 4 tipos de armas canónicas:
    1. *Dagger:* Rápida, startup/recovery mínimos, bajo daño y corto alcance.
    2. *Sword:* Equilibrada en velocidad, daño y alcance.
    3. *Spear:* Largo alcance espacial, hitbox estrecha, daño medio y recovery pronunciado.
    4. *Hammer:* Startup y recovery lentos, alto consumo de estamina, pero daño masivo y gran impulso de knockback.
  * Implementar la proyección de un vector continuo y estandarizado `[f32; 8]` para acondicionar las observaciones de los agentes en MARL (**FR-06**), alineado 1:1 con los atributos de arma en `AgentCombatFeaturesPOD`.

#### 2. Especificación Técnica y Contrato
* **Rutas / Componentes:**
  * `src/combat/mod.rs`
  * `src/combat/weapon.rs`
  * `assets/configs/weapons/dagger.json`
  * `assets/configs/weapons/sword.json`
  * `assets/configs/weapons/spear.json`
  * `assets/configs/weapons/hammer.json`
* **Lógica Principal:**
  * **Estructura DTO de Ingestión (`WeaponConfigDTO`):**
    * Campos parseados vía Serde: `id: String`, `weapon_type: String`, `startup_ticks: u32`, `active_ticks: u32`, `recovery_ticks: u32`, `base_damage: f32`, `stamina_cost: f32`, `knockback_impulse: f32`, `blockstun_ticks: u32`, `hitstun_ticks: u32`, `hitbox_offset: Vector2D`, `hitbox_size: Vector2D`.
  * **Estructura Runtime POD (`WeaponConfigPOD`):**
    * Declarada estrictamente con `repr(C, align(64))` con un tamaño exacto de 64 bytes, empaquetada descendentemente para eliminar todo padding implícito:
      1. `id: [u8; 16]` (Offset 0..16, 16 bytes, align 1): Identificador alfanumérico zero-padded.
      2. `hitbox_offset: Vector2D` (Offset 16..24, 8 bytes, align 8): Desplazamiento relativo al origen del luchador.
      3. `hitbox_size: Vector2D` (Offset 24..32, 8 bytes, align 8): Anchura y altura del área de impacto.
      4. `base_damage: f32` (Offset 32..36, 4 bytes, align 4): Daño base infligido.
      5. `stamina_cost: f32` (Offset 36..40, 4 bytes, align 4): Consumo instantáneo de estamina al iniciar ataque.
      6. `knockback_impulse: f32` (Offset 40..44, 4 bytes, align 4): Magnitud de impulso de retroceso impartido al oponente.
      7. `max_reach: f32` (Offset 44..48, 4 bytes, align 4): Alcance longitudinal precomputado (`hitbox_offset.x + hitbox_size.x`) para inferencia ultra-rápida.
      8. `startup_ticks: u16` (Offset 48..50, 2 bytes, align 2): Ticks de preparación sin daño.
      9. `active_ticks: u16` (Offset 50..52, 2 bytes, align 2): Ticks de activación de hitbox.
      10. `recovery_ticks: u16` (Offset 52..54, 2 bytes, align 2): Ticks de vulnerabilidad tras impacto.
      11. `hitstun_ticks: u16` (Offset 54..56, 2 bytes, align 2): Duración de incapacitación impartida a la víctima.
      12. `blockstun_ticks: u16` (Offset 56..58, 2 bytes, align 2): Duración de incapacitación impartida si el impacto fue bloqueado.
      13. `weapon_type: u8` (Offset 58..59, 1 byte, align 1): Discriminante de tipo de arma (0=Dagger, 1=Sword, 2=Spear, 3=Hammer).
      14. `_pad: [u8; 5]` (Offset 59..64, 5 bytes, align 1): Relleno explícito zero-initialized completando exactamente 64 bytes.
   * **Validaciones Numéricas e Invariantes:**
    * Restricciones de frames: `startup_ticks >= 1`, `active_ticks >= 1`, `recovery_ticks >= 1`, `hitstun_ticks >= 1`, `blockstun_ticks >= 1`.
    * Restricciones físicas: `base_damage > 0.0`, `stamina_cost >= 0.0`, `knockback_impulse >= 0.0`.
    * Geometría: `hitbox_size.x > 0.0`, `hitbox_size.y > 0.0`.
    * Supresión de Flotantes Subnormales y No-Finitos: Para cada campo flotante escalar `x`, debe cumplirse `x == 0.0 || (x.is_finite() && x.is_normal())`. Se rechazan estrictamente `NaN`, `+Infinity`, `-Infinity` y magnitudes subnormales ($0 < |x| < 1.17549435 \times 10^{-38}$). Si `x == 0.0`, se fuerza canónicamente a `+0.0f32` (`0x00000000`).
  * **API de Carga del Catálogo (`load_weapon_catalog`):**
    * Función pública en `src/combat/weapon.rs`:
      `pub fn load_weapon_catalog(dir: &std::path::Path) -> Result<[WeaponConfigPOD; 4], WeaponConfigError>`
      Carga en orden determinista estricto por `weapon_type`: 0=`dagger.json`, 1=`sword.json`, 2=`spear.json`, 3=`hammer.json`.
  * **Vector Estandarizado de Observación (`to_feature_vector`):**
    * Genera un array estático `[f32; 8]` de valores normalizados en $[0.0, 1.0]$ listo para integrarse directamente en `AgentCombatFeaturesPOD`:
      `[startup / 60.0, active / 60.0, recovery / 60.0, damage / 100.0, stamina_cost / 100.0, knockback / 50.0, max_reach / 10.0, weapon_type as f32]`.
* **Manejo de Errores Esperado:**
  * Errores tipados mediante enum de dominio `WeaponConfigError` (derivado con `thiserror::Error` en `src/combat/weapon.rs`):
    - `WeaponConfigError::InvalidFrameData { weapon_id: String, field: &'static str, value: u16 }`: Emitido durante la compilación del DTO si `startup_ticks == 0`, `active_ticks == 0`, `recovery_ticks == 0`, `hitstun_ticks == 0` o `blockstun_ticks == 0`.
    - `WeaponConfigError::InvalidPhysicalBounds { weapon_id: String, field: &'static str, value: f32 }`: Emitido si `base_damage <= 0.0`, `stamina_cost < 0.0`, `knockback_impulse < 0.0`, o si `hitbox_size.x <= 0.0` / `hitbox_size.y <= 0.0`.
    - `WeaponConfigError::SubnormalFloatDetected { weapon_id: String, field: &'static str }`: Emitido si algún parámetro flotante no nulo no cumple `x.is_normal()` o si no es finito (`x.is_nan() || x.is_infinite()`).
    - `WeaponConfigError::WeaponNotFound { weapon_type_id: u8 }`: Emitido al consultar un tipo fuera del rango válido $[0, 3]$.
    - `WeaponConfigError::IoError(std::io::ErrorKind)`: Mapeo de fallos de E/S al leer archivos de configuración desde disco.

#### 3. Criterios de Aceptación y Verificación (DoD)
- [ ] **Configuraciones JSON Creadas y Validadas:** Estructura los 4 archivos de configuración externa (`dagger.json`, `sword.json`, `spear.json`, `hammer.json`) con atributos diferenciados y geométricamente consistentes.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::weapon::tests::test_load_all_weapons_catalog`
- [ ] **Alineación L1 y Tamaño Compile-Time de `WeaponConfigPOD`:** Asegura mediante static assertions que `size_of::<WeaponConfigPOD>() == 64` y `align_of::<WeaponConfigPOD>() == 64`.
  - *Comando / Prueba de Verificación:* `cargo check --no-default-features --features python`
- [ ] **Rechazo de Parámetros Inválidos o Subnormales:** Verifica que armas con ticks en cero, daños negativos o valores flotantes subnormales sean rechazadas con un error tipado.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::weapon::tests::test_weapon_validation_errors`
- [ ] **Generación Invariante del Vector de Atributos:** Valida que el método `to_feature_vector()` produzca un array `[f32; 8]` determinista y sin asignaciones en heap.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::weapon::tests::test_weapon_feature_vector_invariance`

---

### ISSUE-011: [Combat / FSM] Máquina de Estados Finitos de Frame-Data y Ciclo de Vida de Acción

**Metadatos:** Capa: Backend / Core Rust  
**Dependencias Directas:** [ISSUE-007, ISSUE-008, ISSUE-010]

#### 1. Propósito y Razonamiento
* **Secuencia:** Se implementa directamente después de estructurar el catálogo de armas. La FSM requiere conocer la duración de las fases temporales provistas por `WeaponConfigPOD` para orquestar la transición temporal de las entidades.
* **Valor de Negocio:** Cumple el requerimiento central **FR-02** (Lógica de combate basada en Frame-Data). Modela la profundidad táctica y temporal de los juegos de pelea competitivos, garantizando que ninguna acción sea instantánea y que exista compromiso de frames (*startup commitment*) y vulnerabilidad (*recovery whiff*).
* **Riesgos Técnicos Prevenidos:** Evita cancelaciones ilegales de animaciones que rompan la simulación, desbordamientos de enteros en contadores de frames (`tick overflow`), consumo o regeneración ilícita de estamina durante estados de incapacitación y estados zombi en luchadores sin salud.
* **Decisiones de Arquitectura:**
  * Ampliar `FighterState` de 32 a 64 bytes (`repr(C, align(64))`) para alojar de forma gapless y orientada a SIMD: el estado de combate actual (`CombatState`), contadores de ticks de fase, ticks de aturdimiento, flags de bloqueo e identificación del arma equipada.
  * Al expandir `FighterState` a 64 bytes, `WorldState` pasa a ocupar exactamente $64 \times 64 + 32 + 8 + 1 + 23 = 4160$ bytes (65 bloques exactos de 64 bytes de caché L1), manteniendo la estricta alineación `repr(C, align(64))` y el hashing determinista sin paddings dinámicos.
  * Definir `CombatState` como un enum `repr(u8)` con estados exhaustivos:
    - `0 = Idle`, `1 = Moving`, `2 = Startup`, `3 = Active`, `4 = Recovery`, `5 = Blocking`, `6 = Blockstun`, `7 = Hitstun`, `8 = Dodge`, `9 = Dead`.
  * La FSM es estricta: durante `Startup`, `Active`, `Recovery`, `Hitstun` y `Blockstun`, la entidad queda bloqueada para emitir nuevas acciones intencionales, excepto cuando transiciona forzadamente a `Hitstun` por impacto recibido.

#### 2. Especificación Técnica y Contrato
* **Rutas / Componentes:**
  * `src/combat/fsm.rs`
  * `src/combat/state.rs`
  * `src/state.rs` (actualización estructural del POD)
* **Lógica Principal:**
  * **Nuevo Layout de Memoria de `FighterState` (64 bytes exactos, align 64):**
    - `position: Vector2D` (Offset 0..8, 8 bytes, align 8): Coordenadas cartesianas del centro de la entidad.
    - `velocity: Vector2D` (Offset 8..16, 8 bytes, align 8): Vector de velocidad actual.
    - `health: f32` (Offset 16..20, 4 bytes, align 4): Puntos de vida actuales.
    - `stamina: f32` (Offset 20..24, 4 bytes, align 4): Puntos de estamina disponibles.
    - `id: u32` (Offset 24..28, 4 bytes, align 4): Índice único de la entidad en el array global.
    - `state_ticks: u16` (Offset 28..30, 2 bytes, align 2): Contador de ticks transcurridos en el estado FSM actual.
    - `stun_ticks: u16` (Offset 30..32, 2 bytes, align 2): Ticks restantes de incapacitación en Hitstun o Blockstun.
    - `combat_state: u8` (Offset 32..33, 1 byte, align 1): Discriminante de `CombatState`.
    - `equipped_weapon_id: u8` (Offset 33..34, 1 byte, align 1): Índice del arma activa en el registro ($0..3$).
    - `team_id: u8` (Offset 34..35, 1 byte, align 1): Identificador de bando ($0=\text{Team A}, 1=\text{Team B}$).
    - `facing_right: bool` (Offset 35..36, 1 byte, align 1): Orientación horizontal (`true`=derecha, `false`=izquierda).
    - `is_blocking: bool` (Offset 36..37, 1 byte, align 1): Bandera de postura de guardia activa.
    - `_pad_align: [u8; 3]` (Offset 37..40, 3 bytes, align 1): Relleno explícito de alineamiento a frontera de 8 bytes.
    - `hit_entity_mask: u64` (Offset 40..48, 8 bytes, align 8): Bitmask de 64 bits para control de impactos por swing. El bit $k$ indica si la entidad con `id = k` ya recibió impacto durante el swing actual, permitiendo daño en área (cleave) pero impidiendo multihits continuos sobre la misma víctima.
    - `_pad: [u8; 16]` (Offset 48..64, 16 bytes, align 1): Relleno explícito zero-initialized completando exactamente 64 bytes.
  * **Definición de Estados de Combate (`CombatState`):**
    - `0 = Idle`, `1 = Moving`, `2 = Startup`, `3 = Active`, `4 = Recovery`, `5 = Blocking`, `6 = Blockstun`, `7 = Hitstun`, `8 = Dodge`, `9 = Dead`.
  * **Constantes Numéricas Inmutables de Combate (`src/combat/fsm.rs`):**
    - `pub const STAMINA_REGEN_PER_TICK: f32 = 0.25;`
    - `pub const BLOCK_STAMINA_DRAIN_PER_TICK: f32 = 0.05;`
    - `pub const GUARD_BREAK_STUN_TICKS: u16 = 30;`
    - `pub const DODGE_STAMINA_COST: f32 = 15.0;`
    - `pub const DODGE_TOTAL_TICKS: u16 = 12;`
    - `pub const DODGE_IFRAME_START: u16 = 2;`
    - `pub const DODGE_IFRAME_END: u16 = 6;`
    - `pub const DODGE_SPEED_SCALAR: f32 = 1.6;`
  * **Invariante de Cero Padding en `FighterState`:**
    - Los campos de relleno explícito `_pad_align: [u8; 3]` y `_pad: [u8; 16]` DEBEN ser inicializados estrictamente a `0x00` y preservados inmutables durante todas las mutaciones de la FSM, garantizando bit-exactness total en la serialización y en el cálculo hash `xxh3_64`.
  * **Reglas de Transición y Algoritmo de Avance (`fsm_step`):**
    1. **Entidad Derrotada:** Si `health <= 0.0`, forzar `combat_state = Dead`, `velocity = Vector2D::ZERO`, `is_blocking = false`, `hit_entity_mask = 0`, y omitir cualquier procesamiento posterior.
    2. **Recuperación de Aturdimiento (`Hitstun` / `Blockstun`):**
       - Si `combat_state == Hitstun` o `combat_state == Blockstun`: decrementar `stun_ticks` de forma saturada (`stun_ticks = stun_ticks.saturating_sub(1)`). Si `stun_ticks == 0`, transicionar a `Idle` y asegurar `is_blocking = false`.
       - Durante ambos estados, la velocidad voluntaria se fuerza estrictamente a cero; solo se integra la desaceleración multiplicativa por fricción del knockback.
    3. **Ciclo de Vida de Ataque (`Startup` -> `Active` -> `Recovery`):**
       - En todas las fases de ataque, la velocidad voluntaria se inhibe estrictamente (`velocity = Vector2D::ZERO`).
       - Si `combat_state == Startup`: incrementar `state_ticks`. Si `state_ticks >= weapon.startup_ticks`, transicionar a `Active`, resetear `state_ticks = 0` y `hit_entity_mask = 0`.
       - Si `combat_state == Active`: incrementar `state_ticks`. Si `state_ticks >= weapon.active_ticks`, transicionar a `Recovery` y resetear `state_ticks = 0`.
       - Si `combat_state == Recovery`: incrementar `state_ticks`. Si `state_ticks >= weapon.recovery_ticks`, transicionar a `Idle`, resetear `state_ticks = 0` y `hit_entity_mask = 0`.
    4. **Ciclo de Vida de Esquiva (`Dodge`):**
       - Si la acción solicita esquiva desde estado neutral (`Bit 2`): derivar la dirección de avance a partir de `move_intent`. Si `move_intent.length_squared() < 1.0e-6`, asignar vector unitario frontal por defecto: `(1.0, 0.0)` si `facing_right == true`, o `(-1.0, 0.0)` si `facing_right == false`. Fijar `velocity = dir * (move_speed * DODGE_SPEED_SCALAR)`.
       - Si `combat_state == Dodge`: incrementar `state_ticks`. Durante los ticks activos (`DODGE_IFRAME_START..=DODGE_IFRAME_END`), la entidad es invulnerable a colisiones. La velocidad se preserva constante sin ser sobreescrita por inputs de movimiento. Al alcanzar `DODGE_TOTAL_TICKS`, transicionar a `Idle` y resetear `state_ticks = 0`.
    5. **Mantenimiento y Salida de Guardia (`Blocking`):**
       - Si `combat_state == Blocking`:
         - Si `action_flags` NO tiene activo el `Bit 1` (solicitud de guardia liberada): transicionar inmediatamente a `Idle` y asignar `is_blocking = false`.
         - Si `action_flags` mantiene activo el `Bit 1`: permanecer en `Blocking`, forzar velocidad voluntaria a cero, inhibir la regeneración pasiva y restar `BLOCK_STAMINA_DRAIN_PER_TICK` a `stamina`. Si `stamina <= 0.0`, forzar Guard Break inmediato por agotamiento autónomo transicionando a `Hitstun` con `stun_ticks = GUARD_BREAK_STUN_TICKS`, `is_blocking = false` y `stamina = +0.0f32`.
    6. **Estados Neutrales (`Idle` / `Moving`):**
       - Orientación: Si `move_intent.x > 0.0`, asignar `facing_right = true`. Si `move_intent.x < 0.0`, asignar `facing_right = false`. En cualquier otro estado de combate, la orientación queda bloqueada.
       - Si la acción solicita ataque (`Bit 0`): verificar `stamina >= weapon.stamina_cost`. Si cumple, restar estamina, asignar `combat_state = Startup`, `state_ticks = 0`, `is_blocking = false`, `hit_entity_mask = 0`. Si no cumple, rechazar la transición.
       - Si la acción solicita esquiva (`Bit 2`): verificar `stamina >= DODGE_STAMINA_COST`. Si cumple, restar estamina, asignar `combat_state = Dodge`, `state_ticks = 0`, `is_blocking = false`.
       - Si la acción solicita bloqueo (`Bit 1`): asignar `combat_state = Blocking`, `is_blocking = true`.
       - Evaluación Cinética Neutral: si no se emitieron acciones, actualizar entre `Idle` (si `velocity.length_squared() == 0.0`) y `Moving` (si `velocity.length_squared() > 0.0`), asegurando `is_blocking = false`.
       - Regeneración Pasiva de Estamina: Incrementa a razón de `STAMINA_REGEN_PER_TICK` (+0.25 por tick) **exclusivamente** si el estado es `Idle` o `Moving`, acotada a `max_stamina` mediante `min(max_stamina, stamina + STAMINA_REGEN_PER_TICK)`.
* **Manejo de Errores Esperado:**
  * En el bucle de simulación de alta velocidad (`TickEngine::step`), los intentos de emitir acciones incompatibles no generan pánico ni asignan memoria dinámica: se rechazan de forma determinista y silenciosa manteniendo a la entidad en su estado bloqueado.
  * Para APIs de inspección, depuración y tests unitarios, las validaciones de transición retornan `Result<(), CombatFsmError>` (`src/combat/fsm.rs`):
    - `CombatFsmError::ActionLockedByFrameData { entity_id: u32, current_state: u8, requested_action: u32 }`: Emitido al intentar atacar, esquivar o bloquear mientras la entidad se encuentra en `Startup`, `Active`, `Recovery`, `Hitstun` o `Blockstun`.
    - `CombatFsmError::InsufficientStamina { entity_id: u32, current_stamina: f32, required_stamina: f32 }`: Emitido al intentar iniciar un ataque o esquiva sin contar con la estamina mínima requerida.

#### 3. Criterios de Aceptación y Verificación (DoD)
- [ ] **Alineación L1 y Layout de `FighterState` y `WorldState`:** Asegura en compile-time que `size_of::<FighterState>() == 64` y `size_of::<WorldState>() == 4160`, ambos con alineación de 64 bytes.
  - *Comando / Prueba de Verificación:* `cargo check --no-default-features --features python`
- [ ] **Transición Secuencial Estricta de Frame-Data:** Valida mediante test unitario que un ataque transite de forma exacta a lo largo de los ticks por `Startup -> Active -> Recovery -> Idle` según la configuración de cada arma.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::fsm::tests::test_attack_lifecycle_progression`
- [ ] **Bloqueo de Interrupción y Fallo por Estamina Insuficiente:** Comprueba que emitir nuevos ataques durante fases activas sea ignorado, y que intentar atacar con estamina inferior a `stamina_cost` no inicie el `Startup`.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::fsm::tests::test_stamina_and_interruption_lock`
- [ ] **Regeneración de Estamina Condicional:** Verifica que la estamina se regenere exclusivamente en estados neutrales (`Idle`, `Moving`) y nunca durante `Startup`, `Active`, `Recovery` o `Hitstun`.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::fsm::tests::test_stamina_regeneration_states`

---

### ISSUE-012: [Combat / Physics] Detección Continua de Colisiones (Continuous 2D CCD / Swept AABB)

**Metadatos:** Capa: Backend / Core Rust  
**Dependencias Directas:** [ISSUE-011]

#### 1. Propósito y Razonamiento
* **Secuencia:** Una vez que la FSM controla de forma exacta cuándo un luchador entra en la fase `Active`, este issue proyecta las cajas de impacto (*Hitboxes*) y detecta intersecciones físicas con las cajas de vulnerabilidad (*Hurtboxes*) de los oponentes.
* **Valor de Negocio:** Satisface el requerimiento **FR-03** (Detección de colisiones e impactos 2D). Garantiza una experiencia de juego justa y matemáticamente precisa donde las armas de ataque rápido no sufran del problema de "efecto túnel" (*tunneling*), atravesando al adversario entre dos ticks discretos sin colisionar.
* **Riesgos Técnicos Prevenidos:** Elimina el fenómeno de tunneling cinemático a velocidades elevadas, previene auto-daño del atacante, prohíbe colisiones entre aliados (*friendly fire*) en el modo base, y evita la degradación de rendimiento por asignaciones dinámicas en heap en cada paso de simulación.
* **Decisiones de Arquitectura:**
  * Modelar cajas de colisión alineadas a los ejes (`AABB`) definidas por dos puntos: `min: Vector2D` y `max: Vector2D`.
  * La **Hurtbox** del defensor se deriva directamente de su posición `position` y las dimensiones `collider_size` de sus atributos.
  * La **Hitbox** del atacante se activa **únicamente** cuando `combat_state == Active`, desplazando `hitbox_offset` según el vector de orientación `facing_right` y expandiendo las dimensiones `hitbox_size` del arma equipada.
  * Implementar Detección Continua de Colisiones (CCD) mediante el algoritmo **Swept AABB** en espacio relativo. Si los objetos no colisionan al inicio del tick pero sus trayectorias se cruzan durante el desplazamiento del tick actual, calcula el Tiempo de Impacto exacto ($t_{\text{impact}} \in [0.0, 1.0]$).
  * Consolidar los contactos en un buffer estático en el stack: `[ContactPOD; MAX_CONTACTS_PER_TICK]` con capacidad máxima fija de 128 contactos para garantizar cero alocaciones en heap.

#### 2. Especificación Técnica y Contrato
* **Rutas / Componentes:**
  * `src/combat/collision.rs`
  * `src/combat/geometry.rs`
* **Lógica Principal:**
  * **Estructura Geométrica AABB (`AABB`):**
    * Representación POD: `min: Vector2D`, `max: Vector2D`.
    * Funciones booleanas de contención e intersección estática (`intersects(&self, other: &AABB) -> bool`).
   * **Algoritmo Robusto Swept AABB Relativo (`swept_aabb`):**
    * Entradas: `box_a: &AABB`, `vel_a: Vector2D`, `box_b: &AABB`, `vel_b: Vector2D`.
    * **Paso 1 (Intersección Estática en Posición Inicial):**
      - Comprueba si `box_a.intersects(box_b)`.
      - Si se solapan, retorna inmediatamente `Some(ContactPOD)` con $\text{toi} = 0.0f32$. La normal se deriva del eje de mínima penetración entre centros relativos. En caso de singularidad con centros idénticos ($\vec{c}_a == \vec{c}_b$), se asigna canónicamente la normal horizontal $\vec{n} = (1.0, 0.0)$, eliminando cualquier posibilidad de división por cero o generación de `NaN`.
    * **Paso 2 (Barrido Continuo para $t \in (0.0, 1.0]$):**
      - Calcula la traslación relativa en el tick: $\vec{d}_{\text{rel}} = (\vec{v}_a - \vec{v}_b) \cdot \Delta t$.
      - Para cada eje $i \in \{x, y\}$:
        - Si $|d_{\text{rel}, i}| < 1.0 \times 10^{-7}$, evaluar contención estática en el eje; si no hay solapamiento unidimensional en dicho eje, descartar impacto inmediatamente (`None`). Si hay solapamiento, asignar canónicamente $t_{\text{entry}, i} = 0.0f32$ y $t_{\text{exit}, i} = 1.0f32$ (evitando divisiones por cero e infinitos $\pm\infty$).
        - Si $|d_{\text{rel}, i}| \ge 1.0 \times 10^{-7}$, calcular distancias de entrada y salida entre caras opuestas y dividir deterministamente para obtener $t_{\text{entry}, i}$ y $t_{\text{exit}, i}$. Si $t_{\text{entry}, i} > t_{\text{exit}, i}$, conmutar ambos valores.
      - Computa $t_{\text{entry}} = \max(t_{\text{entry}, x}, t_{\text{entry}, y})$ y $t_{\text{exit}} = \min(t_{\text{exit}, x}, t_{\text{exit}, y})$.
      - Criterio de Colisión Válida: Si $t_{\text{entry}} \le t_{\text{exit}}$, $t_{\text{exit}} \ge 0.0$ y $0.0 \le t_{\text{entry}} \le 1.0$: retorna `Some(ContactPOD)` con $\text{toi} = t_{\text{entry}}$.
      - Regla de Desempate en Normal (Tie-Breaking): La normal se orienta en oposición al movimiento relativo en el eje que maximizó $t_{\text{entry}}$. Si $t_{\text{entry}, x} == t_{\text{entry}, y}$ (impacto diagonal perfecto), se asigna precedencia canónica estricta al eje X, garantizando determinismo bit-exacto multiplataforma. En cualquier otro caso, retorna `None`.
  * **Estructura Runtime de Contacto (`ContactPOD`):**
    * Declarada estrictamente como `repr(C, align(32))` de 32 bytes exactos:
      1. `attacker_id: u32` (Offset 0..4, 4 bytes)
      2. `defender_id: u32` (Offset 4..8, 4 bytes)
      3. `toi: f32` (Offset 8..12, 4 bytes): Time Of Impact normalizado $[0.0, 1.0]$.
      4. `penetration: f32` (Offset 12..16, 4 bytes): Magnitud de penetración relativa.
      5. `normal: Vector2D` (Offset 16..24, 8 bytes): Vector normal de la colisión.
      6. `_pad: [u8; 8]` (Offset 24..32, 8 bytes): Relleno explícito zeroed.
  * **Pipeline de Barrido de Colisiones por Tick (`detect_combat_collisions`):**
    1. Itera sobre cada entidad atacante activa en estado `combat_state == Active`.
    2. Construye la Hitbox activa proyectando desde `attacker.position` (centro de masa):
       - En $X$ según `facing_right`:
         - Si `facing_right == true`: $\text{min}_x = \text{pos}_x + \text{offset}_x$, $\text{max}_x = \text{min}_x + \text{size}_x$.
         - Si `facing_right == false`: $\text{max}_x = \text{pos}_x - \text{offset}_x$, $\text{min}_x = \text{max}_x - \text{size}_x$.
       - En $Y$: $\text{min}_y = \text{pos}_y + \text{offset}_y - \text{size}_y \times 0.5$, $\text{max}_y = \text{min}_y + \text{size}_y$.
    3. Itera sobre entidades potenciales receptoras:
       - Ignora si `target.id == attacker.id` (evita auto-daño).
       - Ignora si `target.team_id == attacker.team_id` (filtro estricto de fuego amigo en el modo base).
       - Ignora si `target.health <= 0.0` (entidades derrotadas) o `target.combat_state == Dodge`.
       - Ignora si `(attacker.hit_entity_mask & (1 << target.id)) != 0` (la víctima ya fue impactada en este swing; se evita el multihit continuo preservando el cleave a otros enemigos).
    4. Construye la Hurtbox centrada del defensor:
       - $\text{min} = \text{target.position} - \text{attr.collider\_size} \times 0.5$
       - $\text{max} = \text{target.position} + \text{attr.collider\_size} \times 0.5$
    5. Ejecuta prueba Swept AABB de Hitbox atacante contra Hurtbox defensora utilizando sus velocidades actuales.
    6. Almacena los contactos resultantes en el buffer preasignado `[ContactPOD; 128]`, saturando de forma segura sin exceder la capacidad.
* **Manejo de Errores Esperado:**
  * Gestión física determinista y validaciones mediante enum `CollisionError` (`src/combat/collision.rs`):
    - `CollisionError::ContactBufferOverflow { capacity: usize, detected: usize }`: Cuando los contactos simultáneos exceden la capacidad estática de 128 pares en el stack, el motor satura el buffer de forma segura preservando los 128 contactos con menor TOI y emite una advertencia bajo compilación de test/debug, garantizando cero pánicos y cero alocaciones en heap.
    - `CollisionError::DegenerateColliderBounds { entity_id: u32, field: &'static str, value: f32 }`: Emitido en fases de inicialización/spawn si las dimensiones del collider AABB no son estrictamente positivas o contienen flotantes no finitos.

#### 3. Criterios de Aceptación y Verificación (DoD)
- [ ] **Mitigación Anti-Tunneling Verificada por Swept AABB:** Comprueba mediante test que un ataque veloz con desplazamiento entre ticks que saltaría una Hurtbox en colisión discreta AABB convencional sea detectado con $t_{\text{impact}} \in (0.0, 1.0)$.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::collision::tests::test_swept_aabb_anti_tunneling`
- [ ] **Alineación Geometría e Invarianza Direccional:** Valida que la Hitbox se invierta simétricamente en el eje X cuando `facing_right` conmuta de `true` a `false`.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::collision::tests::test_hitbox_directional_mirroring`
- [ ] **Inmunidad de Fuego Amigo y Auto-Colisión:** Asegura que dos entidades del mismo equipo o una entidad contra su propia Hitbox nunca generen un registro en el buffer de contactos.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::collision::tests::test_friendly_fire_and_self_hit_exclusion`
- [ ] **Cero Alocaciones Dinámicas en Heap Durante CCD:** Ejecuta test con validador de alocación (`assert_no_alloc`) certificando que la fase de detección de colisiones no genera reservas en heap.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::collision::tests::test_collision_pipeline_zero_alloc`

---

### ISSUE-013: [Combat / Resolution] Pipeline de Resolución de Impactos: Daño, Guardia, Stun y Knockback

**Metadatos:** Capa: Backend / Core Rust  
**Dependencias Directas:** [ISSUE-011, ISSUE-012]

#### 1. Propósito y Razonamiento
* **Secuencia:** Procesa los contactos generados por el detector continuo de colisiones (ISSUE-012) y aplica las modificaciones de salud, estamina, aturdimiento y cinemática en las entidades afectadas.
* **Valor de Negocio:** Culmina la implementación funcional de **FR-02** y **FR-03**. Determina el desenlace de los intercambios físicos: castiga al oponente desprotegido mediante daño e incapacitación (*Hitstun*), o premia la lectura defensiva (*Blockstun*) mitigando daño a cambio de desgaste de estamina y retroceso de guardia.
* **Riesgos Técnicos Prevenidos:** Previene que un único swing de ataque conecte daño en múltiples ticks continuos (fenómeno de daño multihit descontrolado), evita divisiones por cero o infinitos en el impulso de knockback al dividir por la masa/peso del objetivo, y garantiza que la salud y estamina no adquieran representaciones de ceros negativos (`-0.0f32`).
* **Decisiones de Arquitectura:**
    * **Regla de Bloqueo Espacial Relativo (Anti Cross-up Bug):** Un defensor mitiga un impacto si y solo si:
      1. Su estado es `Blocking` (`combat_state == Blocking` e `is_blocking == true`).
      2. El defensor está orientado frontalmente hacia la posición $X$ del atacante:
         $$\text{facing\_attacker} = \begin{cases} \text{defender.facing\_right} == \text{true}, & \text{si } \text{attacker.pos.x} \ge \text{defender.pos.x} \\ \text{defender.facing\_right} == \text{false}, & \text{si } \text{attacker.pos.x} < \text{defender.pos.x} \end{cases}$$
         $$\text{guard\_active} = \text{defender.is\_blocking} \land \text{facing\_attacker}$$
    * **Resolución de Bloqueo Exitoso:**
      * Mitigación de daño a salud: se aplica solo el 10% del daño base como *chip damage*.
      * Consumo de estamina del defensor: se resta el 100% del costo de estamina del arma atacante.
      * Si la estamina del defensor cae a $\le 0.0$ por el impacto, se genera un **Guard Break**: el defensor pierde la guardia, transiciona inmediatamente a `Hitstun` con duración aumentada en +50% (`(weapon.hitstun_ticks * 3) / 2`), `is_blocking = false` y estamina canónica en `+0.0f32`.
      * Si la estamina no se agota, el defensor entra en `Blockstun` durante `weapon.blockstun_ticks`.
      * Impulso de knockback: se aplica atenuado al 50% alejando al defensor del atacante.
    * **Resolución de Impacto Limpio (Sin Bloqueo):**
      * Se resta el 100% de `weapon.base_damage` a la salud de la víctima: `health = max(0.0, health - damage)`.
      * La víctima transiciona forzadamente a `Hitstun` durante `weapon.hitstun_ticks`, reseteando `is_blocking = false`.
      * Se aplica vector cinemático de knockback alejando al defensor del origen del atacante:
        $$\text{dir}_x = \begin{cases} 1.0, & \text{si } \text{defender.pos.x} \ge \text{attacker.pos.x} \\ -1.0, & \text{si } \text{defender.pos.x} < \text{attacker.pos.x} \end{cases}$$
        $$\vec{v}_{\text{kb}} = \text{dir}_x \cdot \left(\frac{\text{weapon.knockback\_impulse}}{\max(0.1, \text{defender.weight})}\right) \cdot \vec{u}_x$$
    * **Consumo de Impacto por Víctima (Multi-Agent Cleave):** En el instante en que el atacante conecta sobre un defensor $k$, se actualiza su máscara atómica/bitmask:
      `attacker.hit_entity_mask |= (1 << k)`.
      Esto previene que la víctima $k$ vuelva a recibir daño o stun en ticks activos subsiguientes del mismo ataque, pero permite que otros defensores $j \ne k$ sean impactados si intersectan la Hitbox.

#### 2. Especificación Técnica y Contrato
* **Rutas / Componentes:**
  * `src/combat/resolution.rs`
  * `src/combat/mod.rs`
* **Lógica Principal:**
  * **Ordenamiento Determinista de Contactos:**
    * El buffer de contactos acumulados en el tick se ordena in-place de forma determinista mediante ordenamiento inestable libre de ramas (`sort_unstable_by`), empleando una clave compuesta lexicográfica de causalidad estricta:
      1. Criterio Primario: `toi` en orden ascendente (comparación total IEEE 754 canónica).
      2. Criterio Secundario (Desempate): `attacker_id` en orden numérico ascendente.
      3. Criterio Terciario (Desempate Final): `defender_id` en orden numérico ascendente.
      Esta secuencia garantiza una resolución de causalidad física uniforme e invariante frente a diferencias de optimización de compilador o arquitectura de CPU.
  * **Pipeline Secuencial de Aplicación (`resolve_combat_impacts`):**
    1. Itera sobre los contactos ordenados deterministamente bajo la clave compuesta.
    2. Comprueba si `(attacker.hit_entity_mask & (1 << defender.id)) != 0`. De ser así, descarta el contacto (evita multihit continuo en el mismo swing).
    3. Comprueba si `defender.health <= 0.0` o `defender.combat_state == Dodge`. De ser así, descarta el contacto.
    4. Evalúa la condición de guardia espacial relativa $\text{guard\_active}$.
    5. Ejecuta bifurcación de resolución (Guardia vs Impacto Limpio):
       - En Impacto Limpio, la velocidad horizontal del defensor se sobreescribe con $\vec{v}_{\text{kb}}$ y la velocidad vertical se resetea canónicamente a `+0.0f32`, garantizando retroceso puramente horizontal.
       - En Bloqueo Exitoso, el impulso atenuado al 50% se inyecta en la componente $X$, anulando igualmente la velocidad residual en $Y$.
    6. Aplica canonicalización de ceros flotantes en salud, estamina y velocidades mediante manipulación bitwise, forzando estrictamente el patrón canónico positivo `+0.0f32` (`0x00000000`).
    7. Marca al atacante activando el bit correspondiente: `attacker.hit_entity_mask |= (1 << defender.id)`.
* **Manejo de Errores Esperado:**
  * Errores de resolución tipados mediante enum `ResolutionError` (`src/combat/resolution.rs`):
    - `ResolutionError::InvalidEntityWeight { entity_id: u32, weight: f32 }`: Emitido si los atributos físicos de la víctima registran una masa corrupta $\le 0.0$ o no-finita. En el paso crítico de integración, el motor aplica branchless clamp preventivo: `max(0.1f32, weight)`.
    - `ResolutionError::EntityNotFound { entity_id: u32 }`: Emitido si un contacto procesado hace referencia a un slot de entidad inactivo (`entity_id as usize >= active_count`). En la simulación se descarta el contacto de forma segura sin abortar el tick.

#### 3. Criterios de Aceptación y Verificación (DoD)
- [ ] **Resolución de Impacto Limpio:** Verifica que un ataque no bloqueado reduzca la salud en la magnitud exacta de `base_damage`, asigne `Hitstun` por los ticks de arma y aplique knockback proporcional al peso.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::resolution::tests::test_clean_hit_damage_and_hitstun`
- [ ] **Mecánica de Bloqueo y Guard Break:** Valida que un bloqueo frontal mitigue el daño a salud, descuente estamina y asigne `Blockstun`; comprueba que al agotar la estamina se aplique Guard Break hacia `Hitstun` extendido.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::resolution::tests::test_guard_mitigation_and_guard_break`
- [ ] **Invariante de Único Impacto por Swing:** Comprueba que un ataque cuya fase `Active` dura múltiples ticks (ej. 5 ticks) solo aplique daño y knockback una única vez a un defensor que permanece dentro del área.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::resolution::tests::test_single_hit_per_swing_invariant`
- [ ] **Canonicalización de Flotantes Tras Resolución:** Certifica que cuando la salud o estamina caen a cero, el valor en memoria sea estrictamente `+0.0f32` canónico (bits `0x00000000`), sin ceros negativos.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::resolution::tests::test_resolution_canonical_zero_floats`

---

### ISSUE-014: [Combat / Engine] Integración en `TickEngine`, Weapon Swapping en `reset()` y Proyección de Observación

**Metadatos:** Capa: Backend / Core Rust  
**Dependencias Directas:** [ISSUE-009, ISSUE-010, ISSUE-011, ISSUE-012, ISSUE-013]

#### 1. Propósito y Razonamiento
* **Secuencia:** Constituye el hito final de integración de la Fase 2. Ensambla todos los subsistemas desarrollados (catálogo de armas, FSM de frame-data, CCD de colisiones y pipeline de resolución) dentro del bucle de simulación `TickEngine::step()`, e implementa el método de reasignación dinámica de equipamiento en `reset()`.
* **Valor de Negocio:** Cumple de manera integral los requerimientos **FR-01** ($N$ vs $M$ táctico), **FR-05** (intercambio dinámico de armas entre episodios sin recreación de memoria) y provee la base para **FR-06** (vector de observación generalizable condicionado por el equipamiento actual).
* **Riesgos Técnicos Prevenidos:** Evita fugas de memoria, alocaciones dinámicas al reiniciar episodios, mutaciones del tamaño de los tensores de observación al cambiar de arma, y desincronizaciones de determinismo al alternar equipamientos en caliente.
* **Decisiones de Arquitectura:**
  * Estructurar el método `TickEngine::step()` en un pipeline determinista estricto de 6 fases secuenciales sin asignación en heap:
    1. *Fase 0 (Validación Fail-Fast):* Validación transaccional de `action_flags` e IDs. Si una acción es inválida, se aborta el tick retornando `Err(EngineError::InvalidEntityId)` sin mutar el estado.
    2. *Fase 1 (FSM de Combate y Compromiso de Acción):* Avance de estados temporales, regeneración pasiva y consumo de estamina. Bloqueo de orientación e inmovilización voluntaria durante fases de compromiso.
    3. *Fase 2 (Cinemática y Despacho Exhaustivo de Velocidad con Fricción):*
       - `Idle` / `Moving`: $\vec{v} = \text{normalize}(\text{move\_intent}) \cdot \text{move\_speed}$.
       - `Dodge`: $\vec{v}$ mantiene su velocidad constante de esquiva ($\text{dir} \cdot \text{move\_speed} \cdot \text{DODGE\_SPEED\_SCALAR}$) fijada en el tick de inicio.
       - `Hitstun` / `Blockstun`: Velocidad voluntaria nula. La velocidad residual de knockback decae multiplicativamente: $\vec{v}_{t+1} = \vec{v}_t \cdot \text{KNOCKBACK\_DECAY\_FACTOR}$ (0.85).
       - `Startup` / `Active` / `Recovery` / `Blocking` / `Dead`: Velocidad forzada estrictamente a `Vector2D::ZERO`.
       - Supresión Flush-To-Zero (FTZ): En cualquier estado, si $|\vec{v}| < 1.0 \times 10^{-4}$, se fuerza canónicamente a `+0.0f32` (`Vector2D::ZERO`).
    4. *Fase 3 (Detección Continua CCD Swept AABB):* Detección de contactos entre Hitboxes activas y Hurtboxes utilizando las posiciones iniciales $\vec{p}_t$ y las velocidades efectivas del tick sobre el diferencial temporal canónico `TICK_DT` ($1.0 / 60.0$), almacenando los contactos en el buffer estático del stack `[ContactPOD; 128]`.
    5. *Fase 4 (Resolución de Impactos y Asignación de Impulsos):* Ordenamiento determinista de contactos (`sort_unstable_by`). Aplicación de mitigación de guardia, guard breaks, actualización de daño, aturdimiento e inyección del vector de impulso de knockback directamente en la velocidad de la víctima. Actualización atómica de `hit_entity_mask`.
    6. *Fase 5 (Integración Espacial, Clamping de Límites y Finalización):* Integración de posición Euler $\vec{p}_{t+1} = \vec{p}_t + \vec{v}_{t+1} \cdot \text{TICK\_DT}$. Clamping branchless contra los bordes de la arena considerando `collider_size`. En cualquier eje donde ocurra colisión con pared, la velocidad correspondiente se fuerza canónicamente a `+0.0f32`. Incremento de `current_tick` y exportación de PRNG.
  * Extender la firma de reset con compatibilidad regresiva completa:
    - Firma principal: `pub fn reset_with_weapons(&mut self, new_seed: u64, weapon_map: &[(u32, u8)]) -> Result<(), EngineError>`
    - Sobrecarga compatible (Phase 1): `pub fn reset(&mut self, new_seed: u64) -> Result<(), EngineError>`  
      (Delega contractualmente en `reset_with_weapons` suministrando un slice de reasignación vacío `&[]`, conservando las armas equipadas previamente o asignando el tipo base por defecto).
  * La reasignación de armas actualiza `equipped_weapon_id` en las entidades correspondientes en tiempo constante $O(1)$, preservando invariante el tamaño del estado global `WorldState` y de las estructuras POD.
  * Diseñar la función `export_agent_combat_features(agent_id)` que produce un vector de longitud fija listo para consumo en Phase 4 (`rust-numpy` / PyO3).

#### 2. Especificación Técnica y Contrato
* **Rutas / Componentes:**
  * `src/engine.rs`
  * `src/combat/features.rs`
  * `tests/test_combat_integration.rs`
* **Lógica Principal:**
  * **Constantes Numéricas Globales del Motor (`src/engine.rs` / `src/combat/resolution.rs`):**
    - `pub const TICK_DT: f32 = 1.0 / 60.0;` (Paso temporal canónico a 60 Hz).
    - `pub const KNOCKBACK_DECAY_FACTOR: f32 = 0.85;` (Coeficiente multiplicativo de fricción).
  * **Contrato de Reasignación en Reset (`reset_with_weapons`):**
    * Valida que cada tupla `(entity_id, weapon_type_id)` referencie un índice de entidad activo ($< \text{active\_count}$) y un tipo de arma existente en el catálogo ($0 \le \text{weapon\_type} \le 3$).
    * Re-inicializa `WorldState` limpiando a cero absoluto (`0x00`) las ranuras de entidades inactivas y restaurando `health = max_health`, `stamina = max_stamina`, `combat_state = Idle`, `state_ticks = 0`, `stun_ticks = 0`, `hit_entity_mask = 0`.
    * Asigna el nuevo `equipped_weapon_id` a cada entidad especificada en el mapa. Si una entidad activa no está presente en `weapon_map`, conserva su arma previa o se inicializa con el arma 0 (`Dagger`).
    * Re-siembra el generador pseudoaleatorio `DeterministicRng` con `new_seed`.
   * **Catálogo Contiguo de Armas en `TickEngine`:**
    * `TickEngine` aloja de forma contigua e inline:
      - `arena: ArenaConfigPOD`
      - `attributes: [FighterAttributesPOD; MAX_SIMULTANEOUS_ENTITIES]`
      - `weapon_registry: [WeaponConfigPOD; 4]`
      - `state: WorldState`
      - `rng: DeterministicRng`
      Garantiza cero indirecciones de memoria, eliminación de heap allocations y lectura en caché L1 ($O(1)$) mediante el discriminante `equipped_weapon_id`.
  * **Estructura POD Unificada de Observación (`AgentCombatFeaturesPOD`):**
    * Declarada estrictamente como `repr(C, align(64))` de 64 bytes exactos (16 floats normalizados), satisfaciendo integralmente **FR-06** para tensores PyTorch sin transformaciones intermedias:
      1. `weapon_startup: f32` (Offset 0..4)
      2. `weapon_active: f32` (Offset 4..8)
      3. `weapon_recovery: f32` (Offset 8..12)
      4. `weapon_damage: f32` (Offset 12..16)
      5. `weapon_stamina_cost: f32` (Offset 16..20)
      6. `weapon_knockback: f32` (Offset 20..24)
      7. `weapon_range: f32` (Offset 24..28, proyecta el valor normalizado de `WeaponConfigPOD.max_reach`)
      8. `weapon_type_id: f32` (Offset 28..32)
      9. `current_state_id: f32` (Offset 32..36)
      10. `current_state_ticks: f32` (Offset 36..40)
      11. `stun_ticks_remaining: f32` (Offset 40..44)
      12. `is_blocking_flag: f32` (Offset 44..48, `1.0` si bloquea, `0.0` si no)
      13. `normalized_health: f32` (Offset 48..52, `health / max_health`)
      14. `normalized_stamina: f32` (Offset 52..56, `stamina / max_stamina`)
      15. `facing_direction: f32` (Offset 56..60, `+1.0` si derecha, `-1.0` si izquierda)
      16. `cleave_hit_count: f32` (Offset 60..64, número de víctimas impactadas derivado de `hit_entity_mask.count_ones() as f32` si el estado es `Active` o `Recovery`; forzado canónicamente a `+0.0f32` en cualquier otro estado neutral o defensivo)
* **Manejo de Errores Esperado:**
  * Extensión directa del enum existente `EngineError` (`src/engine.rs`) implementado en la Fase 1:
    - `EngineError::InvalidWeaponReassignment { entity_id: u32, weapon_type: u8 }`: Retornado por `reset_with_weapons` cuando `weapon_type > 3` (fuera del catálogo de 4 armas canónicas).
    - `EngineError::EntityInactive { entity_id: u32 }`: Retornado por `reset_with_weapons` al intentar asignar equipamiento a un índice de luchador que supera el número de entidades activas (`entity_id as usize >= active_count`).
    - `EngineError::CorruptedCombatState { entity_id: u32, state_id: u8 }`: Retornado en validaciones si una entidad presenta un discriminante de `CombatState` no reconocido ($> 9$), impidiendo la ejecución del paso hasta restaurar un estado canónico.

#### 3. Criterios de Aceptación y Verificación (DoD)
- [ ] **Reasignación Dinámica de Armas en Reset (FR-05):** Valida que invocar `reset_with_weapons` actualice los parámetros de armamento de los agentes seleccionados sin modificar el layout de memoria ni requerir nuevas alocaciones en heap.
  - *Comando / Prueba de Verificación:* `cargo test --test test_combat_integration test_dynamic_weapon_swapping_on_reset`
- [ ] **Simulación Integral de Intercambio de Combate (End-to-End):** Ejecuta un test de integración donde un luchador ataca a un oponente a distancia de alcance, conecta en la fase `Active`, inflige daño, provoca knockback y completa su fase de `Recovery` hasta volver a `Idle`.
  - *Comando / Prueba de Verificación:* `cargo test --test test_combat_integration test_full_combat_engagement_lifecycle`
- [ ] **Determinismo Absoluto en Simulación de Combate:** Verifica que dos simulaciones independientes de combate $2 \text{ vs } 2$ con armas variadas y la misma semilla generen exactamente el mismo hash de estado `xxh3_64` a lo largo de 5,000 ticks continuos.
  - *Comando / Prueba de Verificación:* `cargo test --test test_combat_integration test_combat_determinism_5000_ticks`
- [ ] **Extracción Invariante de Features de Combate (FR-06):** Certifica que `export_agent_combat_features` devuelva un POD de tamaño y alineación constantes sin importar cuál de las 4 armas se encuentre equipada.
  - *Comando / Prueba de Verificación:* `cargo test --lib combat::features::tests::test_combat_features_shape_invariance`