/**
 * ✅ 1. 자신에게 전송 (응답 / 확인 등)
 * @param {Socket} socket - 현재 소켓
 * @param {string} type - 응답 타입 (ex: 'chatMessage')
 * @param {any} data - 응답 데이터
 */
exports.sendToSelf = (socket, type, data) => {
    socket.emit(type, {
      status: 200,
      data,
    });
  };
  
  /**
   * ✅ 2. 자신에게 에러 전송
   * @param {Socket} socket - 현재 소켓
   * @param {string} type - 응답 타입
   * @param {number} status - 에러 상태 코드 (ex: 400, 401, 500)
   * @param {string} error - 에러 메시지
   */
  exports.sendErrorToSelf = (socket, type, status, error) => {
    socket.emit(type, {
      status,
      error,
    });
  };
  
  /**
   * ✅ 3. 방 전체에게 전송 (자신 포함)
   * @param {Server} io - Socket.IO 서버 인스턴스
   * @param {string} roomId - 방 ID
   * @param {string} type - 이벤트 타입
   * @param {any} data - 보낼 데이터
   */
  exports.sendToRoom = (io, roomId, type, data) => {
    io.to(roomId).emit(type, {
      status: 200,
      data,
    });
  };
  
  /**
   * ✅ 4. 방 전체에게 에러 전송
   * @param {Server} io - Socket.IO 서버 인스턴스
   * @param {string} roomId - 방 ID
   * @param {string} type - 이벤트 타입
   * @param {number} status - 에러 상태
   * @param {string} error - 에러 메시지
   */
  exports.sendErrorToRoom = (io, roomId, type, status, error) => {
    io.to(roomId).emit(type, {
      status,
      error,
    });
  };
  
  /**
   * ✅ 5. 방 내 '자신 제외' 유저에게 전송 (브로드캐스트용)
   * @param {Socket} socket - 현재 소켓
   * @param {string} roomId - 방 ID
   * @param {string} type - 이벤트 타입
   * @param {any} data - 보낼 데이터
   */
  exports.sendToRoomExceptSelf = (socket, roomId, type, data) => {
    socket.to(roomId).emit(type, {
      status: 200,
      data,
    });
  };
  
  /**
   * ✅ 6. 특정 소켓 ID에게 전송
   * @param {Server} io - Socket.IO 서버 인스턴스
   * @param {string} socketId - 대상 소켓 ID
   * @param {string} type - 이벤트 타입
   * @param {any} data - 보낼 데이터
   */
  exports.sendToSocketId = (io, socketId, type, data) => {
    io.to(socketId).emit(type, {
      status: 200,
      data,
    });
  };
  
  /**
   * ✅ 7. 특정 소켓 ID에게 에러 전송
   * @param {Server} io - Socket.IO 서버 인스턴스
   * @param {string} socketId - 대상 소켓 ID
   * @param {string} type - 이벤트 타입
   * @param {number} status - 에러 상태 코드
   * @param {string} error - 에러 메시지
   */
  exports.sendErrorToSocketId = (io, socketId, type, status, error) => {
    io.to(socketId).emit(type, {
      status,
      error,
    });
  };
  