const express = require('express');
const router = express.Router();
const roomListController = require('../controllers/roomListController');
const {validateCreateRoom} = require('../middlkewares/joi');

router.post('/create', validateCreateRoom, roomListController.createRoom);
router.get('/getList', roomListController.getRoomList);
router.post('/join', roomListController.joinRoom);
router.post('/leave', roomListController.leaveRoom);

module.exports = router;