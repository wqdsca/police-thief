const path = require('path');
require('dotenv').config({ path: path.join(__dirname, '../.env') });
const express = require('express');
const http = require('http'); // 추후 https모듈 사용
const app = express();
const apiRouter = require('../Handlers/Api/routes/ApiRouter');
const corsMiddleware = require('../Handlers/Api/middlkewares/cors');
const {applyRateLimiter} = require('../Handlers/Api/middlkewares/rateLimiter');
const {connectDB} = require('../Shared/config/db');
const TokenService = require('../Shared/service/Token/tokenService');

//전역 밀드웨어 적용
app.use(corsMiddleware);
applyRateLimiter(app);
app.use(express.json());
app.use(express.urlencoded({ extended: true }));
// 로그인, 회원가입에는 토큰인증 빼기
const noTokenRoutes = ['/auth/login', '/register'];
app.use('/api', (req, res, next) => {
    // if(noTokenRoutes.includes(req.path)) {
    //     next();
    // } else {
    //     TokenService.verifyAccessToken(req, res, next); //개발용 토큰없이 개발중
    // }
    next();
})
app.use('/api', apiRouter);

const PORT = process.env.API_PORT || 3000;
const server = http.createServer(app);
server.listen(PORT,'127.0.0.1', async () => {   // 보안문제로 개발시 127.0.0.1로 설정
    await connectDB();
    console.log(`✅ API 서버가 포트 ${PORT}에서 실행 중입니다.`);
});

// Graceful shutdown
process.on('SIGINT', () => {
    console.log('\n🛑 API 서버 종료 중...');
    server.close(() => {
        console.log('✅ API 서버가 안전하게 종료되었습니다.');
        process.exit(0);
    });
});

process.on('SIGTERM', () => {
    console.log('\n🛑 API 서버 종료 중...');
    server.close(() => {
        console.log('✅ API 서버가 안전하게 종료되었습니다.');
        process.exit(0);
    });
});


