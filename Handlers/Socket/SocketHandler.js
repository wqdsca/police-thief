const {socketBaseController} = require('./Controller/SocketBaseController');
const connectionService = require('./service/connectionService');
const TokenService = require('../../Shared/service/Token/TokenService');
const connectionController = require('./Controller/ConnectionController');
module.exports = (io) => {
    io.on('connection', async (socket) => {
    //     TokenService.verifySocketToken(socket, async (err) => {
    //         if (err) {
    //             console.error(`유효하지 않은 토큰입니다. ${err.message}`);
    //             socket.disconnect();
    //             return;
    //           }
    //           const id = socket.handshake.query.id;
    //     try {
    //         await connectionController.handleConnection(socket, id);
    //     } catch (error) {
    //         console.error(error);
    //     }
    // });
    // 아래는 테스트용 코드 작성 토큰 검증없이 id값만 확인
    try{
        const id = socket.handshake.query.id;
        await connectionController.handleConnection(socket, id);
    } catch (error) {
        console.error(error);
    }
    socket.on('type', async(msg) => {
        const {type, data} = msg;
        // case별로 쪼개서 보내기
        switch(type) {
            case '1' : //하트비트 수신보내기
                await connectionController.handleHartbeat(data);
                break;
            // case '2' : // 방 입장
            //     await connectionController.handleRoomEnter(data);
            //     break;
        }
    }); 
    socket.on('disconnect',  () => {
        console.log('소켓연결 해제');
     connectionController.handleDisconnect(socket, data);
    });
    });
}