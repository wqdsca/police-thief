const { SocketBaseService } = require("./socketBaseService");
const redisUserService = require("../../../Shared/service/redis/redisUserService");
const BaseRepository = require("../../../Shared/repository/BaseRepository");

class ConnectionService  extends SocketBaseService {
    constructor() {
        super(BaseRepository, redisUserService, console);
    }

    async handleConnection(id, socketId) {
        if(!id || !socketId) {
            throw new Error("id 또는 socketId가 없습니다.");
        }
        await this.redisService.set(id, "socketId", socketId, 60 * 30);
        this.logger.info(`${id} 연결 성공`);
        return true;
    }

    async handleDisconnect( id) {
        if(!id) {
            throw new Error("id가 없습니다.");
        }
        await this.redisService.deleteList(id);
        this.logger.info(`${id} 연결 종료`);
    }
    
    // 하트비트 수신
    async handleHartbeat(data) {
        const {id} = data;
        try {
            this.logger.info(`${id} 하트비트 수신`);
            await this.redisService.updateTTL(id, 60 * 30); //하트비트 수신했을때 30분 연장
            return true;
        } catch (error) {
            this.logger.error(`하트비트 처리 중 오류 발생: ${error.message}`);
            return false;
        }
    }
}

module.exports = new ConnectionService();