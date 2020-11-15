fn main() {
    Pow { base: 2, exp: 5 }.do_thing()
}

pub fn my_pow(base: u32, mut exp: u32) -> u32 {
    let mut output = 1;
    while exp > 0 {
        output *= base;
        exp -= 1;
    }
    output
}

pub trait WillDo {
    fn do_thing(&self);
}

pub struct Pow {
    base: u32,
    exp: u32,
}

impl WillDo for Pow {
    fn do_thing(&self) {
        let result = my_pow(self.base, self.exp);
        println!("pow({}, {}) = {}", self.base, self.exp, result);
    }
}
