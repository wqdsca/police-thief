const express = require('express');
const router = express.Router();

const authRouter = require('./authRouter');
const registerRouter = require('./registerRouter');
const roomListRouter = require('./roomListRouter');

router.use('/auth', authRouter);
router.use('/register', registerRouter);
router.use('/roomList', roomListRouter);

module.exports = router;