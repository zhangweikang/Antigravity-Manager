import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Card,
  Space,
  Button,
  Input,
  Table,
  Tag,
  Modal,
  Form,
  message,
  Tooltip,
  Switch,
  Typography,
  Empty,
  Popconfirm,
} from "antd";
import {
  PlusOutlined,
  DeleteOutlined,
  ReloadOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  KeyOutlined,
  SearchOutlined,
  QuestionCircleOutlined,
  LoginOutlined,
  GlobalOutlined,
} from "@ant-design/icons";
import type { ColumnsType } from "antd/es/table";
import { request as invoke } from "../utils/request";

const { Text, Link } = Typography;

interface PerplexityAccount {
  id: string;
  name: string | null;
  email?: string | null;
  enabled: boolean;
  proxy_enabled: boolean;
  created_at: number;
  last_used: number;
  auth_type?: 'api_key' | 'web_session'; // 认证类型
}

interface PerplexityConfig {
  enabled: boolean;
  api_key: string | null;
  default_model: string;
}

const Perplexity = () => {
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<PerplexityAccount[]>([]);
  const [config, setConfig] = useState<PerplexityConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [addModalVisible, setAddModalVisible] = useState(false);
  const [configModalVisible, setConfigModalVisible] = useState(false);
  const [searchText, setSearchText] = useState("");
  const [form] = Form.useForm();
  const [configForm] = Form.useForm();
  const [webLoginLoading, setWebLoginLoading] = useState(false);
  const [cookieSubmitModalVisible, setCookieSubmitModalVisible] = useState(false);
  const [cookieForm] = Form.useForm();

  // 加载配置
  const loadConfig = async () => {
    try {
      const result = await invoke<PerplexityConfig>('perplexity_get_config');
      setConfig(result);
      form.setFieldsValue(result); // Sync Add Account form defaults if needed, though mostly for config form
      configForm.setFieldsValue(result); // Sync Config form
    } catch (error) {
      console.error("Failed to load Perplexity config:", error);
      message.error(t("perplexity.loadConfigFailed", "Failed to load configuration"));
    }
  };

  // 加载账号列表
  const loadAccounts = async () => {
    setLoading(true);
    try {
      const result = await invoke<PerplexityAccount[]>('perplexity_list_accounts');
      // 后端返回的 PerplexityWebAccount 字段略有不同，这里直接使用，因为 interface 兼容
      // 后端: enabled, proxy_enabled, name, id, email, created_at, last_used
      setAccounts(result);
    } catch (error) {
      console.error("Failed to load Perplexity accounts:", error);
      message.error(t("perplexity.loadFailed", "Failed to load accounts"));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadConfig();
    loadAccounts();
  }, []);

  // 添加账号
  const handleAddAccount = async (values: {
    name: string;
    api_key: string;
  }) => {
    try {
      // TODO: 实现后端 API Key 账号添加
      // await invoke('add_perplexity_account', { name: values.name, apiKey: values.api_key });
      message.warning("API Key account support coming soon. Please use Web Login.");
      // message.success(t("perplexity.addSuccess", "Account added successfully"));
      setAddModalVisible(false);
      form.resetFields();
      loadAccounts();
    } catch (error) {
      message.error(t("perplexity.addFailed", "Failed to add account"));
    }
  };

  // 删除账号
  const handleDeleteAccount = async (id: string) => {
    try {
      await invoke('perplexity_delete_account', { id });
      message.success(t("perplexity.deleteSuccess", "Account deleted"));
      loadAccounts();
    } catch (error) {
      message.error(t("perplexity.deleteFailed", "Failed to delete account"));
    }
  };

  // 切换启用状态
  const handleToggleEnabled = async (id: string, enabled: boolean) => {
    try {
      // TODO: 实现后端 API
      // await invoke('toggle_perplexity_account', { id, enabled });
      message.warning("Toggling individual accounts is not yet supported in backend.");
      // message.success(
      //   enabled
      //     ? t("perplexity.enabled", "Account enabled")
      //     : t("perplexity.disabled", "Account disabled"),
      // );
      loadAccounts();
    } catch (error) {
      message.error(t("perplexity.toggleFailed", "Failed to toggle account"));
    }
  };

  // 保存配置
  const handleSaveConfig = async (values: {
    enabled: boolean;
    api_key: string;
    default_model: string;
  }) => {
    try {
      await invoke('perplexity_save_config', { config: values });
      message.success(t("perplexity.configSaved", "Configuration saved"));
      setConfigModalVisible(false);
      loadConfig();
    } catch (error) {
      console.error("Save config error", error);
      message.error(
        t("perplexity.configSaveFailed", "Failed to save configuration"),
      );
    }
  };

  // Web 登录
  const handleWebLogin = async () => {
    setWebLoginLoading(true);
    try {
      // 打开 Perplexity 登录页面
      await invoke<string>('perplexity_start_login');
      message.info(t("perplexity.webLoginStarted", "Please login in the browser window, then paste your cookies below."));
      // 打开 Cookie 提交弹窗
      setCookieSubmitModalVisible(true);
    } catch (error) {
      message.error(t("perplexity.webLoginFailed", "Failed to start web login"));
    } finally {
      setWebLoginLoading(false);
    }
  };

  // 提交 Cookie
  const handleSubmitCookies = async (values: { name: string; cookies: string }) => {
    try {
      await invoke('perplexity_submit_cookies', { name: values.name, cookies: values.cookies });
      message.success(t("perplexity.addSuccess", "Account added successfully"));
      setCookieSubmitModalVisible(false);
      cookieForm.resetFields();
      loadAccounts();
    } catch (error) {
      message.error(t("perplexity.invalidCookies", "Invalid or expired cookies. Please try logging in again."));
    }
  };

  // 取消登录
  const handleCancelWebLogin = () => {
    invoke('perplexity_cancel_login');
    setCookieSubmitModalVisible(false);
    cookieForm.resetFields();
  };

  // 表格列定义
  const columns: ColumnsType<PerplexityAccount> = [
    {
      title: t("perplexity.name", "Name"),
      dataIndex: "name",
      key: "name",
      render: (name: string | null) => name || "-",
    },
    {
      title: t("perplexity.status", "Status"),
      dataIndex: "enabled",
      key: "enabled",
      render: (enabled: boolean) => (
        <Tag
          color={enabled ? "success" : "default"}
          icon={enabled ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
        >
          {enabled
            ? t("common.enabled", "Enabled")
            : t("common.disabled", "Disabled")}
        </Tag>
      ),
    },
    {
      title: t("perplexity.proxyEnabled", "Proxy"),
      dataIndex: "proxy_enabled",
      key: "proxy_enabled",
      render: (enabled: boolean, record) => (
        <Switch
          checked={enabled}
          onChange={(checked) => handleToggleEnabled(record.id, checked)}
          size="small"
        />
      ),
    },
    {
      title: t("perplexity.lastUsed", "Last Used"),
      dataIndex: "last_used",
      key: "last_used",
      render: (timestamp: number) => {
        if (!timestamp) return "-";
        return new Date(timestamp * 1000).toLocaleString();
      },
    },
    {
      title: t("common.actions", "Actions"),
      key: "actions",
      render: (_, record) => (
        <Space>
          <Popconfirm
            title={t(
              "perplexity.deleteConfirm",
              "Are you sure you want to delete this account?",
            )}
            onConfirm={() => handleDeleteAccount(record.id)}
            okText={t("common.yes", "Yes")}
            cancelText={t("common.no", "No")}
          >
            <Button type="text" danger icon={<DeleteOutlined />} size="small" />
          </Popconfirm>
        </Space>
      ),
    },
  ];

  // 过滤账号
  const filteredAccounts = accounts.filter(
    (account) =>
      !searchText ||
      (account.name &&
        account.name.toLowerCase().includes(searchText.toLowerCase())),
  );

  return (
    <div className="p-6">
      <Card
        title={
          <Space>
            <img
              src="/perplexity-icon.svg"
              alt="Perplexity"
              style={{ width: 24, height: 24 }}
              onError={(e) => {
                e.currentTarget.style.display = "none";
              }}
            />
            <span>{t("perplexity.title", "Perplexity")}</span>
            <Tooltip
              title={t(
                "perplexity.description",
                "Manage Perplexity AI accounts and API keys",
              )}
            >
              <QuestionCircleOutlined style={{ color: "#999" }} />
            </Tooltip>
          </Space>
        }
        extra={
          <Space>
            <Button
              icon={<KeyOutlined />}
              onClick={() => {
                configForm.setFieldsValue(config);
                setConfigModalVisible(true);
              }}
            >
              {t("perplexity.configure", "Configure")}
            </Button>
            <Button
              icon={<GlobalOutlined />}
              onClick={handleWebLogin}
              loading={webLoginLoading}
            >
              {t("perplexity.webLogin", "Web Login")}
            </Button>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => setAddModalVisible(true)}
            >
              {t("perplexity.addAccount", "Add API Key")}
            </Button>
          </Space>
        }
      >
        {/* 搜索和操作栏 */}
        <div className="mb-4 flex justify-between">
          <Input
            placeholder={t(
              "perplexity.searchPlaceholder",
              "Search accounts...",
            )}
            prefix={<SearchOutlined />}
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            style={{ width: 300 }}
            allowClear
          />
          <Button
            icon={<ReloadOutlined />}
            onClick={loadAccounts}
            loading={loading}
          >
            {t("common.refresh", "Refresh")}
          </Button>
        </div>

        {/* 状态提示 */}
        {config && !config.enabled && (
          <div className="mb-4 p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg">
            <Text type="warning">
              ⚠️{" "}
              {t(
                "perplexity.notEnabled",
                'Perplexity proxy is not enabled. Click "Configure" to enable it.',
              )}
            </Text>
          </div>
        )}

        {/* 账号表格 */}
        {accounts.length === 0 ? (
          <Empty
            description={
              <div>
                <p>
                  {t(
                    "perplexity.noAccounts",
                    "No Perplexity accounts configured",
                  )}
                </p>
                <p className="text-gray-400 text-sm mt-2">
                  {t("perplexity.getApiKey", "Get your API key from")}{" "}
                  <Link
                    href="https://www.perplexity.ai/settings/api"
                    target="_blank"
                  >
                    perplexity.ai/settings/api
                  </Link>
                </p>
              </div>
            }
          >
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => setAddModalVisible(true)}
            >
              {t("perplexity.addFirstAccount", "Add First Account")}
            </Button>
          </Empty>
        ) : (
          <Table
            columns={columns}
            dataSource={filteredAccounts}
            rowKey="id"
            loading={loading}
            pagination={{ pageSize: 10 }}
          />
        )}
      </Card>

      {/* 添加账号弹窗 */}
      <Modal
        title={t("perplexity.addAccount", "Add Perplexity Account")}
        open={addModalVisible}
        onCancel={() => {
          setAddModalVisible(false);
          form.resetFields();
        }}
        footer={null}
      >
        <Form form={form} onFinish={handleAddAccount} layout="vertical">
          <Form.Item
            name="name"
            label={t("perplexity.accountName", "Account Name")}
            rules={[
              {
                required: true,
                message: t(
                  "perplexity.nameRequired",
                  "Please enter account name",
                ),
              },
            ]}
          >
            <Input
              placeholder={t(
                "perplexity.namePlaceholder",
                "e.g., My Perplexity Account",
              )}
            />
          </Form.Item>
          <Form.Item
            name="api_key"
            label={t("perplexity.apiKey", "API Key")}
            rules={[
              {
                required: true,
                message: t("perplexity.apiKeyRequired", "Please enter API key"),
              },
            ]}
          >
            <Input.Password placeholder="pplx-xxxxxxxxxxxxxxxx" />
          </Form.Item>
          <Form.Item>
            <Space>
              <Button type="primary" htmlType="submit">
                {t("common.add", "Add")}
              </Button>
              <Button onClick={() => setAddModalVisible(false)}>
                {t("common.cancel", "Cancel")}
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>

      {/* 配置弹窗 */}
      <Modal
        title={t("perplexity.configuration", "Perplexity Configuration")}
        open={configModalVisible}
        onCancel={() => setConfigModalVisible(false)}
        footer={null}
      >
        <Form form={configForm} onFinish={handleSaveConfig} layout="vertical">
          <Form.Item
            name="enabled"
            label={t("perplexity.enableProxy", "Enable Perplexity Proxy")}
            valuePropName="checked"
          >
            <Switch />
          </Form.Item>
          <Form.Item
            name="api_key"
            label={t("perplexity.globalApiKey", "Global API Key (Optional)")}
            tooltip={t(
              "perplexity.globalApiKeyTooltip",
              "Fallback API key used when no account is available",
            )}
          >
            <Input.Password placeholder="pplx-xxxxxxxxxxxxxxxx" />
          </Form.Item>
          <Form.Item
            name="default_model"
            label={t("perplexity.defaultModel", "Default Model")}
          >
            <Input placeholder="sonar" />
          </Form.Item>
          <Form.Item>
            <Space>
              <Button type="primary" htmlType="submit">
                {t("common.save", "Save")}
              </Button>
              <Button onClick={() => setConfigModalVisible(false)}>
                {t("common.cancel", "Cancel")}
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>

      {/* Cookie 提交弹窗 */}
      <Modal
        title={t("perplexity.submitCookies", "Submit Cookies")}
        open={cookieSubmitModalVisible}
        onCancel={handleCancelWebLogin}
        footer={null}
      >
        <div className="mb-4 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
          <div className="flex">
            <LoginOutlined className="mr-2 mt-1" />
            <Text style={{ whiteSpace: 'pre-wrap' }}>
              {t("perplexity.cookieInstructions", "Recommended:\n1. Press F12 to open Developer Tools, switch to [Network] tab\n2. Refresh page, click any request (e.g. session)\n3. Find 'Cookie' in [Request Headers]\n4. Right click Cookie value -> Copy value")}
            </Text>
          </div>
        </div>
        <Form form={cookieForm} onFinish={handleSubmitCookies} layout="vertical">
          <Form.Item
            name="name"
            label={t("perplexity.accountName", "Account Name")}
            rules={[{ required: true, message: t("perplexity.nameRequired", "Please enter account name") }]}
          >
            <Input placeholder={t("perplexity.namePlaceholder", "e.g., My Perplexity Account")} />
          </Form.Item>
          <Form.Item
            name="cookies"
            label={t("perplexity.cookies", "Cookies")}
            rules={[{ required: true, message: t("perplexity.cookiesRequired", "Please paste your cookies") }]}
          >
            <Input.TextArea
              rows={4}
              placeholder="__cf_bm=xxx; __Secure-next-auth.session-token=xxx; ..."
            />
          </Form.Item>
          <Form.Item>
            <Space>
              <Button type="primary" htmlType="submit">
                {t("common.submit", "Submit")}
              </Button>
              <Button onClick={handleCancelWebLogin}>
                {t("common.cancel", "Cancel")}
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
};

export default Perplexity;
