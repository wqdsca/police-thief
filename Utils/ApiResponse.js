const successResponse = (res, statusCode, data) => {
    return res.status(statusCode).json({
        status: statusCode,
        data: data ? data: {},
    });
}

    const errorResponse = (res, statusCode, message) => {
        return res.status(statusCode).json({
            status:statusCode, error: message,
        });
    }

module.exports = {
    successResponse,
    errorResponse,
}