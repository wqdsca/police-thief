const path = require('path');
require('dotenv').config({ path: path.join(__dirname, '../.env') });
const dgram = require('dgram');
const server = dgram.createSocket('udp4');
const redisUserService = require('../Shared/service/redis/redisUser');

async function createUdpServer(){ 
    // 환경변수 검증
    const Udp_IP = process.env.Udp_IP;
    const Udp_PORT = parseInt(process.env.Udp_PORT);
    
    if (!Udp_IP || !Udp_PORT) {
        throw new Error('UDP_IP 또는 UDP_PORT 환경변수가 설정되지 않았습니다.');
    }

    server.on('message',  async (msg, rinfo) => {
        if (rinfo.address !== '127.0.0.1' && rinfo.address !== '::1') {   //보안문제로 127.0.0.1로 설정 외부접근차단
            console.warn(`❌ 외부 IP 접근 차단: ${rinfo.address}`);
            return; // 외부 IP는 무시
        }
        try {
            const header = msg.readUInt8(0);
            if(header === 0x00) {
                /*
                0x00: 헤더 rinfo 저장용, 겜 스타트 시작 용
                */
               const roomId = msg.readInt8(1);
               const playerId = msg.readInt8(2);
               
               console.log(`UDP 메시지 수신: roomId=${roomId}, playerId=${playerId} from ${rinfo.address}:${rinfo.port}`);
               
               if(roomId !== null && playerId !== null) { 
                    const user = await redisUserService.add(roomId, playerId);
                    if(user) {
                        const response = Buffer.alloc(1);
                        response.writeUInt8(0x00, 0);
                        server.send(response, 0, 1, rinfo.port, rinfo.address);
                        console.log(`사용자 추가 성공: roomId=${roomId}, playerId=${playerId}`);
                    } else {
                        // 실패 시 에러 응답
                        const errorResponse = Buffer.alloc(1);
                        errorResponse.writeUInt8(0xFF, 0); // 에러 코드
                        server.send(errorResponse, 0, 1, rinfo.port, rinfo.address);
                        console.error(`사용자 추가 실패: roomId=${roomId}, playerId=${playerId}`);
                    }
               } else {
                   // 잘못된 데이터
                   const errorResponse = Buffer.alloc(1);
                   errorResponse.writeUInt8(0xFF, 0);
                   server.send(errorResponse, 0, 1, rinfo.port, rinfo.address);
                   console.error(`잘못된 데이터: roomId=${roomId}, playerId=${playerId}`);
               }
            } else {
                console.warn(`알 수 없는 헤더: 0x${header.toString(16)}`);
            }
        } catch (error) {
            console.error('UDP 메시지 처리 중 에러:', error);
            // 에러 응답 전송
            const errorResponse = Buffer.alloc(1);
            errorResponse.writeUInt8(0xFF, 0);
            server.send(errorResponse, 0, 1, rinfo.port, rinfo.address);
        }
    });

    server.on('listening', () => {
        const address = server.address();
        console.log(`✅ UDP 서버가 ${address.address}:${address.port}에서 실행 중입니다.`);
    });

    server.on('error', (err) => {
        console.error('❌ UDP 서버 에러:', err);
        process.exit(1);
    });

    try {
        server.bind(Udp_PORT, Udp_IP);
        console.log(`🔄 UDP 서버 바인딩 시도: ${Udp_IP}:${Udp_PORT}`);
    } catch (error) {
        console.error('❌ UDP 서버 바인딩 실패:', error);
        process.exit(1);
    }
}

// 서버 시작
createUdpServer().catch(error => {
    console.error('❌ UDP 서버 시작 실패:', error);
    process.exit(1);
});

// Graceful shutdown
process.on('SIGINT', () => {
    console.log('\n🛑 UDP 서버 종료 중...');
    server.close(() => {
        console.log('✅ UDP 서버가 안전하게 종료되었습니다.');
        process.exit(0);
    });
});

process.on('SIGTERM', () => {
    console.log('\n🛑 UDP 서버 종료 중...');
    server.close(() => {
        console.log('✅ UDP 서버가 안전하게 종료되었습니다.');
        process.exit(0);
    });
});