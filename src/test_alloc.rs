use alloc_counter::AllocCounterSystem;

#[global_allocator]
static GLOBAL: AllocCounterSystem = AllocCounterSystem;
