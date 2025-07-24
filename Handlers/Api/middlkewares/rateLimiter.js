/*
요청 속도 제한
windowsMs : 5초 동안 최대 요청 허용 횟수
*/

const rateLimiter = require('express-rate-limit');

const generateRateLimiter = () => {
    return rateLimiter({
        windowMs: 5 * 1000,
        max: 10,
        message: {
            error: '너무 많은 요청입니다. 잠시 후 다시 시도해주세요'
        }
    });
};

const applyRateLimiter = (app) => {
    app.use("/api", generateRateLimiter());
};

module.exports = { applyRateLimiter };