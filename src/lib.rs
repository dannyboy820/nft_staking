pub fn two() -> i32 { 2 }

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn two_is_two() {
        assert_eq!(2, two())
    }
}