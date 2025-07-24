const { successResponse, errorResponse } = require('../../../Utils/ApiResponse');
const roomListService = require('../services/roomListService');


exports.createRoom = async (req, res) => {
    try {
        const room = await roomListService.createRoom(req.body);
        return successResponse(res, 200, room);
    } catch (error) {
        console.log(error);
        return errorResponse(res, 500, error.message);
    }
}

exports.getRoomList = async (req, res) => {
    try {
        const {limit, lastRoomId} = req.query;
        const roomList = await roomListService.getRoomList(limit, lastRoomId);
        return successResponse(res, 200, roomList);
    } catch (error) {
        console.log(error);
        return errorResponse(res, 500, error.message);
    }
}

exports.joinRoom = async (req, res) => {
    try {
        const {roomId, userId, nickName} = req.body;
        const joinRoom = await roomListService.joinRoom(roomId, userId, nickName);
        return successResponse(res, 200, joinRoom);
    } catch (error) {
        console.log(error);
        return errorResponse(res, 500, error.message);
    }
}

exports.leaveRoom = async (req, res) => {
    try {
        const {roomId, userId} = req.body;
        const leaveRoom = await roomListService.leaveRoom(roomId, userId);
        return successResponse(res, 200, leaveRoom);
    } catch (error) {
        console.log(error);
        return errorResponse(res, 500, error.message);
    }
}

