class SocketBaseService {
    constructor(repository, redisService, logger, options = {}) {
        this.repository = repository;
        this.redisService = redisService;
        this.logger = console;
        this.options = options;
    }

    // 하트비트 수신
    async handleHartbeat(data) {
        const {id} = data;
        try {
            return true;
        } catch (error) {
            this.logger.error('하트비트 처리 중 오류 발생${error.message}');
            return false;
        }
    }

    // 연결 종료
    async handleDisconnect(data) {
        const {id} = data;
        try {
            return true;
        } catch (error) {
            this.logger.error('연결 종료 처리 중 오류 발생${error.message}');
            return false;
        }
    }
    async findSocketId(userId) {
        
    }
    
}