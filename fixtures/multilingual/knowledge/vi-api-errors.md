---
type: Support Article
title: Mã lỗi trả về của Partner API
description: Partner API trả lỗi dưới dạng JSON gồm trường code và message.
lang: vi
---
# Mã lỗi trả về của Partner API

Partner API trả lỗi dưới dạng JSON gồm trường code và message. HTTP 400 kèm code INVALID_PARAM nghĩa là tham số sai định dạng; 401 UNAUTHORIZED là token hết hạn hoặc không hợp lệ; 403 FORBIDDEN khi khóa API không có quyền với tài nguyên; 404 NOT_FOUND khi mã đơn hàng không tồn tại; 409 DUPLICATE_ORDER khi gửi trùng mã tham chiếu đơn. Lỗi 5xx là sự cố phía Hikari, đối tác nên thử lại sau. Mỗi phản hồi lỗi có request_id, hãy gửi kèm mã này khi liên hệ đội hỗ trợ kỹ thuật.
