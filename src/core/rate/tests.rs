#[cfg(test)]
mod tests {
    use super::super::rate::Rate;
    use std::time::Duration;

    #[test]
    fn test_rate_per_second() {
        let rate = Rate::per_second(10);
        assert_eq!(rate.period(), Duration::from_millis(100));

        let rate = Rate::per_second(1);
        assert_eq!(rate.period(), Duration::from_secs(1));
    }

    #[test]
    fn test_rate_per_minute() {
        let rate = Rate::per_minute(60);
        assert_eq!(rate.period(), Duration::from_secs(1));

        let rate = Rate::per_minute(1);
        assert_eq!(rate.period(), Duration::from_secs(60));
    }

    #[test]
    fn test_rate_per_hour() {
        let rate = Rate::per_hour(3600);
        assert_eq!(rate.period(), Duration::from_secs(1));

        let rate = Rate::per_hour(1);
        assert_eq!(rate.period(), Duration::from_secs(3600));
    }

    #[test]
    fn test_rate_per_day() {
        let rate = Rate::per_day(86400);
        assert_eq!(rate.period(), Duration::from_secs(1));

        let rate = Rate::per_day(1);
        assert_eq!(rate.period(), Duration::from_secs(86400));
    }

    #[test]
    fn test_rate_from_count_and_period() {
        // 10 requests per 60 seconds = 1 request per 6 seconds
        let rate = Rate::from_count_and_period(10, 60);
        assert_eq!(rate.period(), Duration::from_secs(6));

        // 30 requests per 60 seconds = 1 request per 2 seconds
        let rate = Rate::from_count_and_period(30, 60);
        assert_eq!(rate.period(), Duration::from_secs(2));

        // Edge case: invalid parameters
        let rate = Rate::from_count_and_period(0, 60);
        assert_eq!(rate.period(), Duration::from_secs(u64::MAX));

        let rate = Rate::from_count_and_period(10, 0);
        assert_eq!(rate.period(), Duration::from_secs(u64::MAX));
    }

    #[test]
    fn test_custom_rate() {
        let custom_period = Duration::from_millis(250);
        let rate = Rate::new(custom_period);
        assert_eq!(rate.period(), custom_period);
    }
}