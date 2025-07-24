const path = require('path');
require('dotenv').config({path: path.resolve(__dirname, '../.env')});
const express = require('express');
const http = require('http'); // 추후 https로 바꿔야함
const {Server} = require('socket.io');
const {connectDB} = require('../Shared/config/db');


const app = express();
const server = http.createServer(app);
const io = new Server(server);

const startSocketServer = async () => {
    const app = express();
    const server = http.createServer(app);
    const io = new Server(server);

    // 데이터베이스 연결
    await connectDB();
    socketHandler(io);
    const PORT = process.env.SOCKET_PORT;
    server.listen(PORT, () => {
        console.log(`Socket server is running on port ${PORT}`);
    });
    
    
}