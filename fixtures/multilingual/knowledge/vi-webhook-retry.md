---
type: Support Article
title: Cơ chế gửi lại webhook
description: Khi có sự kiện như đơn hàng mới hoặc thay đổi trạng thái giao hàng, Hikari gửi webhook dạng POST đến URL mà đối tác đã đăng ký.
lang: vi
---
# Cơ chế gửi lại webhook

Khi có sự kiện như đơn hàng mới hoặc thay đổi trạng thái giao hàng, Hikari gửi webhook dạng POST đến URL mà đối tác đã đăng ký. Máy chủ của đối tác phải trả về mã HTTP 2xx trong vòng 10 giây, nếu không lần gửi bị xem là thất bại. Webhook thất bại được gửi lại tối đa 8 lần với khoảng cách tăng dần: 1 phút, 5 phút, 30 phút, 2 giờ và kéo dài đến 24 giờ. Sau lần thử cuối, sự kiện bị đánh dấu thất bại và có thể gửi lại thủ công trong trang quản trị. Mỗi sự kiện có event_id để đối tác tự lọc bản trùng.
