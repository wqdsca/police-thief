const Joi = require('joi');

function createValidator(schema) {
    return (req, res, next) => {
      const { error } = schema.validate(req.body);
      if (error) {
        return next({ status: 415, message: error.message });
      }
      next();
    };
  }

  const userSchema = Joi.object( {
    loginType: Joi.string().valid('Google', 'Apple', 'Kakao', 'test').required(),
    token: Joi.string().required(),
  });

  const registerSchema = Joi.object({
    nickname: Joi.string().min(3).max(10).required(),
  });
  const createRoomSchema = Joi.object({
    roomName: Joi.string().required().max(20).min(3),
    maxUserNum: Joi.number().required().min(2).max(10),
    userId: Joi.number().required(),
    nickName: Joi.string().required().max(10).min(3),
  });


module.exports = {
    validateUser: createValidator(userSchema),
    validateRegister: createValidator(registerSchema),
    validateCreateRoom: createValidator(createRoomSchema),
}
/** 
 * 데이터 검증
 */