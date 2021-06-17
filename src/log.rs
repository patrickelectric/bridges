#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => ({
        println!($($arg)*);
    })
}
