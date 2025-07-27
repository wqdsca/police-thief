const BaseService = require('../../../Shared/service/BaseService');
const BaseRepository = require('../../../Shared/repository/BaseRepository');
const redisRoomListService = require('../../../Shared/service/redis/redisRoomListService');

class roomListService extends BaseService {
    constructor(repository, redisService, logger, options = {}) {
        super(BaseRepository, redisRoomListService, logger, options);
    }

    async createRoom(data) {
        console.log("createRoom 방만들기 로직 실행, data = ", data);
        const room = await this.redisService.createRoom(data);
        if(!room.success) {
            throw new Error(room.message);
        }
        return room;
    }
    async getRoomList(limit = null, lastRoomId) {
        const roomList = await this.redisService.getRoomList(limit, lastRoomId);
        if(!roomList.success) {
            throw new Error(roomList.message);
        }
        return roomList;
    }
    async joinRoom(roomId, userId, nickName) {
        const joinRoom = await this.redisService.joinRoom({roomId, userId, nickName});
        if(!joinRoom.success) {
            throw new Error(joinRoom.message);
        }
        return joinRoom;
    }
    async leaveRoom(roomId, userId) {
        const exitRoom = await this.redisService.leaveRoom({roomId, userId});
        if(!exitRoom.success) {
            throw new Error(exitRoom.message);
        }
        return exitRoom;
    }
    
}

module.exports = new roomListService();