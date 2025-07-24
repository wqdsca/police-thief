const express = require('express');
const router = express.Router();
const registerController = require('../controllers/registerController');    
const {validateRegister} = require('../middlkewares/joi');

router.post('/register', validateRegister, registerController.register);

module.exports = router;