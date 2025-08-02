use crate::Share::Service::Redis::core::redisGetKey::{KeyType, item_key, list_key, try_get_key};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_keys() {
        assert_eq!(item_key(KeyType::User, 1).unwrap(), "user:1");
        assert_eq!(item_key(KeyType::RoomInfo, 10).unwrap(), "room:list:10");
        assert_eq!(item_key(KeyType::RoomUserList, 9).unwrap(), "room:user:9");
    }

    #[test]
    fn test_list_keys() {
        assert_eq!(list_key(KeyType::RoomInfo).unwrap(), "room:list");
        assert_eq!(list_key(KeyType::RoomUserList).unwrap(), "room:user");
        assert_eq!(list_key(KeyType::RoomListByTime).unwrap(), "room:list:time");
    }

    #[test]
    fn test_custom() {
        assert_eq!(
            try_get_key(KeyType::Custom("enemy".into()), Some(99)).unwrap(),
            "enemy:99"
        );
        assert_eq!(
            try_get_key(KeyType::Custom("cache:match".into()), None).unwrap(),
            "cache:match"
        );
    }

    #[test]
    fn test_error_cases() {
        // User key without id should fail
        assert!(try_get_key(KeyType::User, None).is_err());
        
        // RoomListByTime with id should fail
        assert!(try_get_key(KeyType::RoomListByTime, Some(1)).is_err());
        
        // Custom key should work in both cases
        assert!(try_get_key(KeyType::Custom("test".into()), Some(1)).is_ok());
        assert!(try_get_key(KeyType::Custom("test".into()), None).is_ok());
    }
}