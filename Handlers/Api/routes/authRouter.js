const express = require('express');
const router = express.Router();
const authController = require('../controllers/authController');    
const {validateUser} = require('../middlkewares/joi');

router.post('/login', validateUser, authController.login);

module.exports = router;