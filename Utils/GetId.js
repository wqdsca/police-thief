const { redis } = require('../Shared/config/redis');
const RECYCLED_KEY = 'room:recycled';
const ROOM_COUNTER_KEY = 'room:counter';

async function allocateRoomId() {
  const recycled = await redis.spop(RECYCLED_KEY); // 재사용할 roomId
  if (recycled) return Number(recycled);

  const nextId = await redis.incr(ROOM_COUNTER_KEY); // 새 roomId 발급
  return nextId;
}

async function deleteRoomId(id) {
  if (!id) return;
  await redis.sadd(RECYCLED_KEY, id); // 명시적 종료된 room만 재사용 대상
}

module.exports = {
    allocateRoomId,
    deleteRoomId,
}
