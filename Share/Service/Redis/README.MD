### 1. CacheHelper - LRU 캐시 관리
```rust
use Share::Service::Redis::helpers::CacheHelper;

let cache = CacheHelper::new(conn, Some(60), 50, "room:list", "room:list");

// 아이템 저장 (LRU 자동 관리)
await cache.setItem(123, &user_data)?;

// 아이템 조회
let user = await cache.getItem::<User>(123)?;

// 최근 목록 조회
let recent_ids = await cache.getRecentList(10)?;

// 아이템 제거
await cache.removeItem(123)?;

// 전체 목록 조회
let all_items = await cache.getAllItems()?;

// 목록 크기 조회
let size = await cache.getListSize()?;
```

### 2. HashHelper - Hash 데이터 관리
```rust
use Share::Service::Redis::helpers::HashHelper;

let hash = HashHelper::new(conn, "user:123", Some(3600));

// 단일 필드 설정
await hash.setField("name", "John")?;

// 단일 필드 조회
let name = await hash.getField("name")?;

// 다중 필드 설정
await hash.setMultipleFields(&[("age", "25"), ("city", "Seoul")])?;

// 모든 필드 조회
let all_fields = await hash.getAllFields()?;

// 필드 증가
await hash.incrementField("score", 10)?;

// 필드 삭제
await hash.deleteField("temp_field")?;

// 필드 존재 확인
let exists = await hash.fieldExists("name")?;

// 필드 개수 조회
let count = await hash.getFieldCount()?;

// JSON 전체 저장/조회
await hash.setJson(&user_data)?;
let user = await hash.getJson::<User>()?;
```

### 3. GeoHelper - 위치 기반 데이터 관리
```rust
use Share::Service::Redis::helpers::GeoHelper;

let geo = GeoHelper::new(conn, "players:location", Some(1800));

// 위치 추가
await geo.addLocation(127.0, 37.5, "player:123")?;

// 거리 계산
let distance = await geo.getDistance("player:123", "player:456", "km")?;

// 위치 조회
let positions = await geo.getPositions(&["player:123", "player:456"])?;

// 근처 검색
let nearby = await geo.searchNearby(127.0, 37.5, 3.0, "km", 10, true)?;

// 반경 내 멤버 조회
let members = await geo.getRadiusMembers(127.0, 37.5, 5.0, "km")?;

// 위치 제거
await geo.removeLocation("player:123")?;

// 위치 개수 조회
let count = await geo.getLocationCount()?;
```

### 4. ZSetHelper - 랭킹/정렬 데이터 관리
```rust
use Share::Service::Redis::helpers::ZSetHelper;

let zset = ZSetHelper::new(conn, "leaderboard", Some(86400));

// 멤버 추가
await zset.addMember("player:123", 1500.0)?;

// 멤버 제거
await zset.removeMember("player:123")?;

// 점수 증가
await zset.incrementScore("player:123", 100.0)?;

// 점수 조회
let score = await zset.getScore("player:123")?;

// 상위 랭킹 조회
let top_players = await zset.getTopRankings(0, 9)?;

// 하위 랭킹 조회
let bottom_players = await zset.getBottomRankings(0, 9)?;

// 멤버 개수 조회
let count = await zset.getMemberCount()?;

// 순위 조회
let rank = await zset.getRank("player:123")?;

// 역순 순위 조회
let reverse_rank = await zset.getReverseRank("player:123")?;

// 점수 범위로 멤버 조회
let members = await zset.getMembersByScore(1000.0, 2000.0)?;
```

### 5. SetHelper - Set 데이터 관리
```rust
use Share::Service::Redis::helpers::SetHelper;

let set = SetHelper::new(conn, "online_players", Some(300));

// 단일 멤버 추가
await set.addMember("player:123")?;

// 다중 멤버 추가
await set.addMultipleMembers(&["player:123", "player:456", "player:789"])?;

// 멤버 제거
await set.removeMember("player:123")?;

// 멤버 존재 확인
let exists = await set.memberExists("player:123")?;

// 모든 멤버 조회
let all_members = await set.getAllMembers()?;

// 멤버 개수 조회
let count = await set.getMemberCount()?;

// 랜덤 멤버 추출 (제거됨)
let member = await set.popRandomMember()?;

// 랜덤 멤버들 추출
let members = await set.popRandomMembers(5)?;

// 랜덤 멤버 조회 (제거되지 않음)
let member = await set.getRandomMember()?;

// 랜덤 멤버들 조회
let members = await set.getRandomMembers(5)?;

// 합집합
let union = await set.getUnion(&["set1", "set2"])?;

// 교집합
let intersection = await set.getIntersection(&["set1", "set2"])?;

// 차집합
let difference = await set.getDifference(&["set1", "set2"])?;

// Set 삭제
await set.deleteSet()?;
```

## 🔑 키 생성 시스템

### KeyType 열거형
```rust
pub enum KeyType {
    User,           // "user:{id}"
    RoomInfo,       // "room:list:{id}"
    RoomUserList,   // "room:user:{id}"
    RoomListByTime, // "room:list:time"
    Custom(String), // "{custom}" 또는 "{custom}:{id}"
}
```

### 키 생성 함수들
```rust
use Share::Service::Redis::core::redisGetKey;

// 안전한 키 생성 (권장)
let key = try_get_key(KeyType::User, Some(123))?;

// 편의 함수들
let item_key = item_key(KeyType::User, 123)?;
let list_key = list_key(KeyType::RoomInfo)?;

// 하위 호환 (panic 가능)
let key = get_key(KeyType::User, Some(123));
```