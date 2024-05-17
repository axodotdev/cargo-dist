#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
fn some_fn(input: bool) -> usize {
    if input {
        2 + 4
    } else {
        3_usize.saturating_add(5)
    }
}

#[test]
fn some_test() {
    assert_eq!(some_fn(true), 6);
    assert_eq!(some_fn(false), 8);
}
