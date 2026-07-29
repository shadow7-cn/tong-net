import { Button, Result } from "antd";
import { useNavigate } from "react-router-dom";

export default function NotFound() {
  const navigate = useNavigate();

  return (
    <Result
      status="404"
      title="页面不存在"
      subTitle="这个入口还没有被定义。"
      extra={<Button onClick={() => navigate("/desktop")}>回到互通服务</Button>}
    />
  );
}
