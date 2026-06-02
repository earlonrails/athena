const { Client, LocalAuth } = require('whatsapp-web.js');
const qrcode = require('qrcode-terminal');
const axios = require('axios');
const path = require('path');
const os = require('os');

// Persist session to ~/.athena/whatsapp_session
const sessionPath = path.join(os.homedir(), '.athena', 'whatsapp_session');

const client = new Client({
    authStrategy: new LocalAuth({ dataPath: sessionPath })
});

client.on('qr', (qr) => {
    console.log('Scan the QR code below to pair WhatsApp:');
    qrcode.generate(qr, { small: true });
});

client.on('ready', () => {
    console.log('WhatsApp Client is ready!');
});

client.on('message', async msg => {
    if (msg.from === 'status@broadcast') return;

    try {
        let text = msg.body;
        // In the future: handle audio transcription for voice memos
        // if (msg.hasMedia) { ... }

        if (text) {
            // Forward to local Athena Gateway
            const response = await axios.post('http://localhost:3000/whatsapp/events', {
                message: text
            });

            if (response.data && response.data.response) {
                msg.reply(response.data.response);
            }
        }
    } catch (error) {
        console.error('Error handling message:', error.message);
        msg.reply('Error: Athena gateway unreachable.');
    }
});

client.initialize();
