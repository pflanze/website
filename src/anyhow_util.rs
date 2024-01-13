#[macro_export]
macro_rules! try_anyhow {
    { $e:expr } => {
        $e.map_err(|e| anyhow::anyhow!("try_anyhow: {:?} at {:?} line {}",
                                       e, file!(), line!()))?
    }
}
