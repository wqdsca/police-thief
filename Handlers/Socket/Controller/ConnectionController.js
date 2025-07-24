const connectionService = require('../service/connectionService');
const {sendToSelf} = require('../../../Utils/SocketResponse');
module.exports = {
    handleConnection: async (socket, id) => {
       const result = await connectionService.handleConnection(id, socket.id);
       if(result) {
        socket.emit('connection', 1);
       }
    },
    handleHartbeat: async (socket, data) => {
        const result = await connectionService.handleHartbeat(data);
        if(!result) {
            socket.__heartBeatTimer && clearInterval(socket.__heartBeatTimer);
            console.log('하트비트 처리 중 오류 발생');
    }
    socket.__heartBeatTimer && clearInterval(socket.__heartBeatTimer);
    sendToSelf(socket, '1', null);
    socket.__heartBeatTimer = setInterval(() => {
        sendToSelf(socket, '1', null);
    }, 15 * 60 * 1000); // 15분 마다 하트비트 보내기
    },
    handleDisconnect: async ( data) => {
        const result = await connectionService.handleDisconnect(data);
        if(!result) {
            console.log('연결 종료 처리 중 오류 발생');
        }
    }
    
}