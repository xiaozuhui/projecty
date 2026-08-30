//! JWT 鉴权中间件预留位置。
//!
//! 第一阶段会在这里解析 Bearer Token，将用户身份写入 request extensions，
//! 并由各领域服务组合系统角色、项目成员和部门授权计算最终权限。
