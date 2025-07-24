const registerService = require('../services/registerService');
const {successResponse, errorResponse} = require('../../../Utils/ApiResponse');

exports.register = async (req, res) => {
    try {
        const {nickname} = req.body;
        const user = await registerService.register(nickname);
        return successResponse(res, 200, user);
    } catch (error) {
        return errorResponse(res, 500, error.message);
    }
    }