const authService = require('../services/authService');
const {successResponse, errorResponse} = require('../../../Utils/ApiResponse');

exports.login = async (req, res) => {
    try {
        const {loginType, token} = req.body;
        const user = await authService.login(loginType,token);
        return successResponse(res, 200, user);
    } catch (error) {
        console.log(error);
        return errorResponse(res, 500, error.message);
    }
    }