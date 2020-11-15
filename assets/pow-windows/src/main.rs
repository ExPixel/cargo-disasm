fn main() {
    println!("pow: {}", my_pow(2, 5));
}

pub fn my_pow(base: u32, mut exp: u32) -> u32 {
    let mut output = 1;
    while exp > 0 {
        output *= base;
        exp -= 1;
    }
    output
}
