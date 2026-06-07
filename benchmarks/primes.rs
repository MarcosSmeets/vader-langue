fn is_prime(n: i64) -> bool {
    if n < 2 { return false; }
    let mut i = 2;
    while i * i <= n {
        if n % i == 0 { return false; }
        i += 1;
    }
    true
}
fn main() {
    let mut count = 0i64;
    let mut n = 2i64;
    while n < 2_000_000 {
        if is_prime(n) { count += 1; }
        n += 1;
    }
    println!("{}", count);
}
