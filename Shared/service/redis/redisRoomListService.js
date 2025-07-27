const { getKey } = require('../../../Utils/redisUtils');
const { allocateRoomId } = require('../../../Utils/GetId');
const { getCurrentTime } = require('../../../Utils/Time');
const { pipeline } = require('../../../Utils/redisUtils');
const { zsetHelper, hashHelper } = require('./redisBase');

class RedisRoomListService {
  async createRoom(data) {
    const { roomName, userId, nickName, maxUserNum } = data;
    const roomId = await allocateRoomId();
    const now = getCurrentTime();
    const timestamp = Date.now();

    try {
      const cmds = [
        // 방 정보 저장
        {
          method: 'hset',
          args: [getKey.roomInfo(roomId),
            'roomId', roomId,
            'hostNickName', nickName,
            'roomName', roomName,
            'createdAt', now,
            'maxUserNum', maxUserNum
          ],
        },
        {
          method: 'expire',
          args: [getKey.roomInfo(roomId), 10800], // 3시간
        },

        // 방 유저 목록
        {
          method: 'hset',
          args: [getKey.roomUserList(roomId), userId, JSON.stringify({ nickName })],
        },
        {
          method: 'expire',
          args: [getKey.roomUserList(roomId), 3600],
        },

        // 정렬용 ZSET
        {
          method: 'zadd',
          args: [getKey.roomListByTime(), timestamp, roomId],
        }
      ];


      await pipeline(cmds);

      return {
        roomId,
        success: true,
        message: '방 생성 성공',
      };
    } catch (error) {
      console.error('createRoom 실패:', error);
      return {
        success: false,
        message: '방 생성 실패',
      };
    }
  }

  async getRoomList(limit = 20, lastRoomId) {
    const roomListSet = zsetHelper(() => getKey.roomListByTime());
    const roomListHash = hashHelper((id) => getKey.roomInfo(id));
  
    let max = '+inf';
    console.log('laastroomid  ', lastRoomId);
    if (lastRoomId && Number(lastRoomId) > 0) {
      const score = await roomListSet.score(lastRoomId);
      if (score !== null && !isNaN(score)) {
        max = Number(score) - 1;
        console.log('[getRoomList] max:', max);
      }
    }
  
    const roomIds = await roomListSet.revRangeByScore(max, '0', '20');
  
    if (!roomIds) return [];
  
    const roomListInfo = await Promise.all(
      roomIds.map(async (roomId) => {
        const info = await roomListHash.getAll(roomId);
        return info ? { roomId, ...info } : null;
      })
    );
  
    return {
      roomListInfo,
      success: true,
    };
  }
  

  async joinRoom(data) {
    const { roomId, userId, nickName } = data;
    const roomInfoHash = hashHelper(() => getKey.roomInfo(roomId));
    const roomUserHash = hashHelper(() => getKey.roomUser(roomId));

    const { currentUserNum, maxUserNum } = await roomInfoHash.mget(roomId, 'currentUserNum', 'maxUserNum');
    if (!currentUserNum || !maxUserNum) {
      return { success: false, message: '방 정보 조회 실패' };
    }

    if (Number(currentUserNum) >= Number(maxUserNum)) {
      return { success: false, message: '방 인원 초과' };
    }

    const cmds = [
      {
        method: 'hincrby',
        args: [getKey.roomInfo(roomId), 'currentUserNum', 1],
      },
      {
        method: 'hset',
        args: [getKey.roomUser(roomId), userId, JSON.stringify({ nickName })],
      },
      {
        method: 'expire',
        args: [getKey.roomUser(roomId), 3600],
      }
    ];

    await pipeline(cmds);
    return { success: true, message: '방 참여 성공' };
  }

  async leaveRoom(data) {
    const { roomId, userId } = data;
    const roomInfoHash = hashHelper(() => getKey.roomInfo(roomId));
    const currentUserNum = await roomInfoHash.getField(roomId, 'currentUserNum');

    if (!currentUserNum) {
      return { success: false, message: '방 정보 조회 실패' };
    }

    if (Number(currentUserNum) === 1) {
      await pipeline([
        { method: 'del', args: [getKey.roomUser(roomId)] },
        { method: 'zrem', args: [getKey.roomListByTime(), roomId] },
        { method: 'del', args: [getKey.roomInfo(roomId)] },
      ]);
      return { success: true, message: '방 퇴장 성공, 방 삭제' };
    }

    const cmds = [
      {
        method: 'hincrby',
        args: [getKey.roomInfo(roomId), 'currentUserNum', -1],
      },
      {
        method: 'hdel',
        args: [getKey.roomUser(roomId), userId],
      },
    ];

    await pipeline(cmds);
    return { success: true, message: '방 퇴장 성공' };
  }
}

module.exports = new RedisRoomListService();
